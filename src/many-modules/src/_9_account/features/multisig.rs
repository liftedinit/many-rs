use crate::account::features::{Feature, FeatureId, TryCreateFeature};
use crate::account::Role;
use crate::events::AccountMultisigTransaction;
use crate::ledger::SendArgs;
use crate::EmptyReturn;
use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;
use many_protocol::ResponseMessage;
use many_types::cbor::CborAny;
use many_types::ledger::TokenAmount;
use many_types::{Either, Timestamp};
use minicbor::bytes::ByteVec;
use minicbor::data::Type;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use std::collections::{BTreeMap, BTreeSet};

const MULTISIG_MEMO_DATA_MAX_SIZE: usize = 4000; // 4kB

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
enum MemoInner<const MAX_LENGTH: usize> {
    String(String),
    ByteString(ByteVec),
}

macro_rules! declare_try_from {
    ( $( $ty: ty = $item: path );* $(;)? ) => {
        $(
        impl<const M: usize> TryFrom<$ty> for MemoInner<M> {
            type Error = ManyError;

            fn try_from(value: $ty) -> Result<Self, Self::Error> {
                if value.len() > M {
                    return Err(ManyError::unknown(format!(
                        "Data size ({}) over limit ({})",
                        value.len(),
                        M
                    )));
                }
                Ok($item(value.into()))
            }
        }
        )*
    };
}

declare_try_from!(
    String = Self::String;
    &str = Self::String;
    ByteVec = Self::ByteString;
    Vec<u8> = Self::ByteString;
);

impl<const M: usize> TryFrom<Either<String, ByteVec>> for MemoInner<M> {
    type Error = ManyError;

    fn try_from(value: Either<String, ByteVec>) -> Result<Self, Self::Error> {
        match value {
            Either::Left(str) => Self::try_from(str),
            Either::Right(bstr) => Self::try_from(bstr),
        }
    }
}

impl<C, const M: usize> Encode<C> for MemoInner<M> {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        match self {
            MemoInner::String(str) => e.str(str),
            MemoInner::ByteString(bstr) => e.bytes(bstr.as_slice()),
        }
        .map(|_| ())
    }
}

impl<'b, C, const M: usize> Decode<'b, C> for MemoInner<M> {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        match d.datatype()? {
            Type::Bytes => Self::try_from(d.bytes()?.to_vec()).map_err(decode::Error::message),
            Type::String => Self::try_from(d.str()?).map_err(decode::Error::message),
            // Type::BytesIndef => {}
            // Type::StringIndef => {}
            _ => Err(decode::Error::type_mismatch(Type::String)),
        }
    }
}

/// A memo contains a human readable portion and/or a machine readable portion.
/// It is meant to be a note regarding a message, transaction, info or any
/// type that requires meta information.
#[derive(Clone, Debug, PartialOrd, Eq, PartialEq)]
pub struct Memo<const MAX_LENGTH: usize = MULTISIG_MEMO_DATA_MAX_SIZE> {
    inner: Vec<MemoInner<MAX_LENGTH>>,
}

impl<const M: usize> Memo<M> {
    pub fn push_str(&mut self, str: String) -> Result<(), ManyError> {
        self.inner.push(MemoInner::<M>::try_from(str)?);
        Ok(())
    }

    pub fn push_byte_vec(&mut self, bytes: ByteVec) -> Result<(), ManyError> {
        self.inner.push(MemoInner::<M>::try_from(bytes)?);
        Ok(())
    }
}

impl<const M: usize> From<MemoInner<M>> for Memo<M> {
    fn from(inner: MemoInner<M>) -> Self {
        Self { inner: vec![inner] }
    }
}

impl<const M: usize> TryFrom<Either<String, ByteVec>> for Memo<M> {
    type Error = ManyError;

    fn try_from(s: Either<String, ByteVec>) -> Result<Self, Self::Error> {
        Ok(Self {
            inner: vec![match s {
                Either::Left(str) => MemoInner::String(str),
                Either::Right(bstr) => MemoInner::ByteString(bstr),
            }],
        })
    }
}

impl<const M: usize> TryFrom<String> for Memo<M> {
    type Error = ManyError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(Self::from(MemoInner::<M>::try_from(s)?))
    }
}

impl<const M: usize> TryFrom<ByteVec> for Memo<M> {
    type Error = ManyError;
    fn try_from(b: ByteVec) -> Result<Self, Self::Error> {
        Ok(Self::from(MemoInner::<M>::try_from(b)?))
    }
}

impl<const M: usize> TryFrom<Vec<u8>> for Memo<M> {
    type Error = ManyError;
    fn try_from(b: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(Self::from(MemoInner::<M>::try_from(b)?))
    }
}

impl<C, const M: usize> Encode<C> for Memo<M> {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.encode(&self.inner).map(|_| ())
    }
}

impl<'b, C, const M: usize> Decode<'b, C> for Memo<M> {
    fn decode(d: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, decode::Error> {
        // Allow for backward compatibility when using a feature.
        // We need this if we move a database with existing memos.
        #[cfg(feature = "memo-backward-compatible")]
        match d.datatype()? {
            Type::Bytes => {
                return Self::try_from(d.bytes()?.to_vec()).map_err(decode::Error::message);
            }
            Type::String => {
                return Self::try_from(d.str()?.to_string()).map_err(decode::Error::message);
            }
            _ => {}
        }

        Ok(Self {
            inner: d
                .array_iter_with(ctx)?
                .collect::<Result<Vec<MemoInner<M>>, _>>()?,
        })
    }
}

pub mod errors {
    use many_error::define_attribute_many_error;
    define_attribute_many_error!(
        attribute 9 => {
            100: pub fn transaction_cannot_be_found() => "The transaction cannot be found.",
            101: pub fn user_cannot_approve_transaction() => "The user is not in the list of approvers.",
            102: pub fn transaction_type_unsupported() => "This transaction is not supported.",
            103: pub fn cannot_execute_transaction() => "This transaction cannot be executed yet.",
            104: pub fn transaction_expired_or_withdrawn() => "This transaction expired or was withdrawn.",
        }
    );
}

#[derive(Default, Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct MultisigAccountFeatureArg {
    #[n(0)]
    pub threshold: Option<u64>,

    #[n(1)]
    pub timeout_in_secs: Option<u64>,

    #[n(2)]
    pub execute_automatically: Option<bool>,
}

#[derive(Default)]
pub struct MultisigAccountFeature {
    pub arg: MultisigAccountFeatureArg,
}

impl MultisigAccountFeature {
    pub fn create(
        threshold: Option<u64>,
        timeout_in_secs: Option<u64>,
        execute_automatically: Option<bool>,
    ) -> Self {
        Self::from_arg(MultisigAccountFeatureArg {
            threshold,
            timeout_in_secs,
            execute_automatically,
        })
    }

    pub fn from_arg(arg: MultisigAccountFeatureArg) -> Self {
        Self { arg }
    }
}

impl TryCreateFeature for MultisigAccountFeature {
    const ID: FeatureId = 1;

    fn try_create(f: &Feature) -> Result<Self, ManyError> {
        let argument = f.arguments();
        if argument.len() != 1 {
            return Err(ManyError::invalid_attribute_arguments());
        }

        match argument.get(0) {
            Some(CborAny::Map(m)) => {
                let threshold = m.get(&CborAny::Int(0)).and_then(|v| match v {
                    CborAny::Int(x) => (*x).try_into().ok(),
                    _ => None,
                });
                let timeout_in_secs = m.get(&CborAny::Int(1)).and_then(|v| match v {
                    CborAny::Int(x) => (*x).try_into().ok(),
                    _ => None,
                });
                let execute_automatically = m.get(&CborAny::Int(2)).and_then(|v| match v {
                    CborAny::Bool(x) => Some(*x),
                    _ => None,
                });

                Ok(Self {
                    arg: MultisigAccountFeatureArg {
                        threshold,
                        timeout_in_secs,
                        execute_automatically,
                    },
                })
            }
            _ => Err(ManyError::invalid_attribute_arguments()),
        }
    }
}

impl super::FeatureInfo for MultisigAccountFeature {
    fn as_feature(&self) -> Feature {
        let mut map = BTreeMap::<CborAny, CborAny>::new();
        if let Some(threshold) = self.arg.threshold {
            map.insert(CborAny::Int(0), CborAny::Int(threshold as i64));
        }
        if let Some(timeout_in_secs) = self.arg.timeout_in_secs {
            map.insert(CborAny::Int(1), CborAny::Int(timeout_in_secs as i64));
        }
        if let Some(execute_automatically) = self.arg.execute_automatically {
            map.insert(CborAny::Int(2), CborAny::Bool(execute_automatically));
        }

        Feature::with_id(Self::ID).with_argument(CborAny::Map(map))
    }

    fn roles() -> BTreeSet<Role> {
        BTreeSet::from([Role::CanMultisigSubmit, Role::CanMultisigApprove])
    }
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct SubmitTransactionArgs {
    #[n(0)]
    pub account: Address,

    #[n(1)]
    pub memo: Option<Memo>,

    #[n(2)]
    pub transaction: Box<AccountMultisigTransaction>,

    #[n(3)]
    pub threshold: Option<u64>,

    #[n(4)]
    pub timeout_in_secs: Option<u64>,

    #[n(5)]
    pub execute_automatically: Option<bool>,
}

impl SubmitTransactionArgs {
    pub fn send(from: Address, to: Address, symbol: Address, amount: TokenAmount) -> Self {
        Self {
            account: from,
            memo: None,
            transaction: Box::new(AccountMultisigTransaction::Send(SendArgs {
                from: Some(from),
                to,
                symbol,
                amount,
            })),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
        }
    }
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct SubmitTransactionReturn {
    #[n(0)]
    pub token: ByteVec,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct InfoArgs {
    #[n(0)]
    pub token: ByteVec,
}

#[derive(Clone, Debug, Default, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct ApproverInfo {
    #[n(0)]
    pub approved: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum MultisigTransactionState {
    Pending = 0,
    ExecutedAutomatically = 1,
    ExecutedManually = 2,
    Withdrawn = 3,
    Expired = 4,
}

impl<C> Encode<C> for MultisigTransactionState {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.u8(match self {
            MultisigTransactionState::Pending => 0,
            MultisigTransactionState::ExecutedAutomatically => 1,
            MultisigTransactionState::ExecutedManually => 2,
            MultisigTransactionState::Withdrawn => 3,
            MultisigTransactionState::Expired => 4,
        })?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for MultisigTransactionState {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        match d.u32()? {
            0 => Ok(Self::Pending),
            1 => Ok(Self::ExecutedAutomatically),
            2 => Ok(Self::ExecutedManually),
            3 => Ok(Self::Withdrawn),
            4 => Ok(Self::Expired),
            x => Err(decode::Error::unknown_variant(x)),
        }
    }
}

#[derive(Debug, Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoReturn {
    #[n(0)]
    pub memo: Option<Memo>,

    #[n(1)]
    pub transaction: AccountMultisigTransaction,

    #[n(2)]
    pub submitter: Address,

    #[n(3)]
    pub approvers: BTreeMap<Address, ApproverInfo>,

    #[n(4)]
    pub threshold: u64,

    #[n(5)]
    pub execute_automatically: bool,

    #[n(6)]
    pub timeout: Timestamp,

    #[n(8)]
    pub state: MultisigTransactionState,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct SetDefaultsArgs {
    #[n(0)]
    pub account: Address,

    #[n(1)]
    pub threshold: Option<u64>,

    #[n(2)]
    pub timeout_in_secs: Option<u64>,

    #[n(3)]
    pub execute_automatically: Option<bool>,
}

pub type SetDefaultsReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct ApproveArgs {
    #[n(0)]
    pub token: ByteVec,
}

pub type ApproveReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct RevokeArgs {
    #[n(0)]
    pub token: ByteVec,
}

pub type RevokeReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct ExecuteArgs {
    #[n(0)]
    pub token: ByteVec,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct WithdrawArgs {
    #[n(0)]
    pub token: ByteVec,
}

pub type WithdrawReturn = EmptyReturn;

#[many_module(name = AccountMultisigModule, namespace = account, many_modules_crate = crate)]
pub trait AccountMultisigModuleBackend: Send {
    fn multisig_submit_transaction(
        &mut self,
        sender: &Address,
        args: SubmitTransactionArgs,
    ) -> Result<SubmitTransactionReturn, ManyError>;
    fn multisig_info(&self, sender: &Address, args: InfoArgs) -> Result<InfoReturn, ManyError>;
    fn multisig_set_defaults(
        &mut self,
        sender: &Address,
        args: SetDefaultsArgs,
    ) -> Result<SetDefaultsReturn, ManyError>;
    fn multisig_approve(
        &mut self,
        sender: &Address,
        args: ApproveArgs,
    ) -> Result<ApproveReturn, ManyError>;
    fn multisig_revoke(
        &mut self,
        sender: &Address,
        args: RevokeArgs,
    ) -> Result<RevokeReturn, ManyError>;
    fn multisig_execute(
        &mut self,
        sender: &Address,
        args: ExecuteArgs,
    ) -> Result<ResponseMessage, ManyError>;
    fn multisig_withdraw(
        &mut self,
        sender: &Address,
        args: WithdrawArgs,
    ) -> Result<WithdrawReturn, ManyError>;
}

#[cfg(test)]
mod tests {
    use crate::account::features::multisig::{Memo, MULTISIG_MEMO_DATA_MAX_SIZE};
    use proptest::proptest;

    proptest! {
        #[test]
        fn memo_str_decode_prop(len in 900..1100usize) {
            let data = String::from_utf8(vec![b'A'; len]).unwrap();
            let cbor = format!(r#" [ "{data}" ] "#);
            let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

            let result = minicbor::decode::<Memo<1000>>(&bytes);
            if len <= 1000 {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
            }
        }

        #[test]
        fn memo_bytes_decode_prop(len in 900..1100usize) {
            let data = hex::encode(vec![1u8; len]);
            let cbor = format!(r#" [ h'{data}' ] "#);
            let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

            let result = minicbor::decode::<Memo<1000>>(&bytes);
            if len <= 1000 {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
            }
        }

    }

    #[test]
    fn memo_decode_ok() {
        let data = String::from_utf8(vec![b'A'; MULTISIG_MEMO_DATA_MAX_SIZE]).unwrap();
        let cbor = format!(r#" [ "{data}" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_ok());
    }

    #[test]
    fn memo_decode_too_large() {
        let data = String::from_utf8(vec![b'A'; MULTISIG_MEMO_DATA_MAX_SIZE + 1]).unwrap();
        let cbor = format!(r#" [ "{data}" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }

    #[test]
    fn data_decode_ok() {
        let data = hex::encode(vec![1u8; MULTISIG_MEMO_DATA_MAX_SIZE]);
        let cbor = format!(r#" [ h'{data}' ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_ok());
    }

    #[test]
    fn data_decode_large() {
        let data = hex::encode(vec![1u8; MULTISIG_MEMO_DATA_MAX_SIZE + 1]);
        let cbor = format!(r#" [ h'{data}' ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }

    #[test]
    fn mixed_decode_ok() {
        let data = hex::encode(vec![1u8; MULTISIG_MEMO_DATA_MAX_SIZE]);
        let cbor = format!(r#" [ "", h'{data}', "" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_ok());
    }

    #[test]
    fn mixed_decode_data_too_lare() {
        let data = hex::encode(vec![1u8; MULTISIG_MEMO_DATA_MAX_SIZE + 1]);
        let cbor = format!(r#" [ "", h'{data}', "" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }

    #[test]
    fn mixed_decode_data_type_mismatch() {
        let cbor = r#" [ "", 0, "" ] "#;
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }
}
