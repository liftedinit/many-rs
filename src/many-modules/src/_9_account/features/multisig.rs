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
use many_types::Timestamp;
use minicbor::bytes::ByteVec;
use minicbor::data::Type;
use minicbor::{encode, decode, Decode, Decoder, Encode, Encoder};
use std::collections::{BTreeMap, BTreeSet};

/// A short note in a transaction
#[derive(Clone, Debug, PartialEq)]
// Using AsRef<str> for extensibility:
// We might want to borrow with a Memo<&str> instead of a &Memo<String>
pub struct Memo<S: AsRef<str>>(S);

impl<S, C> Encode<C> for Memo<S> where S: AsRef<str> {
    fn encode<W: encode::Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), encode::Error<W::Error>> {
        e.str(self.0.as_ref()).map(|_| ())
    }
}

impl<C> Decode<'_, C> for Memo<String> {
    fn decode(d: &mut Decoder<'_>, _: &mut C) -> Result<Self, decode::Error> {
        let s = d.str()?;
        if s.as_bytes().len() > MULTISIG_MEMO_DATA_MAX_SIZE {
            return Err(decode::Error::message("Memo size over limit"));
        }
        Ok(Memo(s.into()))
    }
}

// Implementing for completeness, in case we want zero-allocation parsing in the future
impl<'b, C> Decode<'b, C> for Memo<&'b str> {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        let s = d.str()?;
        if s.as_bytes().len() > MULTISIG_MEMO_DATA_MAX_SIZE {
            return Err(decode::Error::message("Memo size over limit"));
        }
        Ok(Memo(s))
    }
}

impl TryFrom<String> for Memo<String> {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.as_str().as_bytes().len() > MULTISIG_MEMO_DATA_MAX_SIZE {
            return Err(format!("Memo size over limit {}", s.as_str().as_bytes().len()));
        }
        Ok(Memo(s))
    }
}

impl Memo<String> {
    // USED ONLY IN TESTS! NEVER REMOVE THIS GUARD! MEMOS SHOULDN'T BE PRODUCED FROM LARGE STRINGS
    #[cfg(test)]
    pub fn new(s: String) -> Self {
        Memo(s)
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

const MULTISIG_MEMO_DATA_MAX_SIZE: usize = 4000; //4kB

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct SubmitTransactionArgs {
    #[n(0)]
    pub account: Address,

    #[n(1)]
    pub memo: Option<Memo<String>>,

    #[n(2)]
    pub transaction: Box<AccountMultisigTransaction>,

    #[n(3)]
    pub threshold: Option<u64>,

    #[n(4)]
    pub timeout_in_secs: Option<u64>,

    #[n(5)]
    pub execute_automatically: Option<bool>,

    #[n(6)]
    #[cbor(decode_with = "decode_data")]
    pub data: Option<ByteVec>,
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
            data: None,
        }
    }
}

/// Data decoder. Check if the data is less than or equal to the maximum allowed size
fn decode_data<C>(
    d: &mut minicbor::Decoder,
    _: &mut C,
) -> Result<Option<ByteVec>, minicbor::decode::Error> {
    match d.datatype()? {
        Type::Bytes => {
            let data = d.bytes()?;
            if data.len() > MULTISIG_MEMO_DATA_MAX_SIZE {
                return Err(minicbor::decode::Error::message("Data size over limit"));
            }
            Ok(Some(data.to_vec().into()))
        }
        Type::Null => {
            d.skip()?;
            Ok(None)
        }
        _ => unimplemented!(),
    }
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct SubmitTransactionReturn {
    #[n(0)]
    pub token: ByteVec,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
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
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        match d.u32()? {
            0 => Ok(Self::Pending),
            1 => Ok(Self::ExecutedAutomatically),
            2 => Ok(Self::ExecutedManually),
            3 => Ok(Self::Withdrawn),
            4 => Ok(Self::Expired),
            x => Err(minicbor::decode::Error::unknown_variant(x)),
        }
    }
}

#[derive(Debug, Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoReturn {
    #[n(0)]
    pub memo: Option<Memo<String>>,

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

    #[n(7)]
    pub data: Option<ByteVec>,

    #[n(8)]
    pub state: MultisigTransactionState,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
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

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct ApproveArgs {
    #[n(0)]
    pub token: ByteVec,
}

pub type ApproveReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct RevokeArgs {
    #[n(0)]
    pub token: ByteVec,
}

pub type RevokeReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct ExecuteArgs {
    #[n(0)]
    pub token: ByteVec,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
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
    use super::SubmitTransactionArgs;
    use crate::account::features::multisig::{MULTISIG_MEMO_DATA_MAX_SIZE, Memo};
    use crate::account::DisableArgs;
    use crate::events::AccountMultisigTransaction;
    use many_identity::testing::identity;

    #[test]
    fn memo_size() {
        let mut tx = SubmitTransactionArgs {
            account: identity(1),
            memo: Some(String::from_utf8(vec![65; MULTISIG_MEMO_DATA_MAX_SIZE]).unwrap().try_into().unwrap()),
            transaction: Box::new(AccountMultisigTransaction::AccountDisable(DisableArgs {
                account: identity(1),
            })),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
            data: None,
        };
        let enc = minicbor::to_vec(&tx).unwrap();
        let dec = minicbor::decode::<SubmitTransactionArgs>(&enc);
        assert!(dec.is_ok());

        tx.memo = Some(Memo::new(String::from_utf8(vec![65; MULTISIG_MEMO_DATA_MAX_SIZE + 1]).unwrap()));
        let enc = minicbor::to_vec(&tx).unwrap();
        let dec = minicbor::decode::<SubmitTransactionArgs>(&enc);
        assert!(dec.is_err());
        assert_eq!(
            dec.unwrap_err().to_string(),
            "decode error: Memo size over limit"
        );
    }

    #[test]
    fn data_size() {
        let mut tx = SubmitTransactionArgs {
            account: identity(1),
            memo: None,
            transaction: Box::new(AccountMultisigTransaction::AccountDisable(DisableArgs {
                account: identity(1),
            })),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
            data: Some(vec![1u8; MULTISIG_MEMO_DATA_MAX_SIZE].into()),
        };
        let enc = minicbor::to_vec(&tx).unwrap();
        let dec = minicbor::decode::<SubmitTransactionArgs>(&enc);
        assert!(dec.is_ok());

        tx.data = Some(vec![1u8; MULTISIG_MEMO_DATA_MAX_SIZE + 1].into());
        let enc = minicbor::to_vec(&tx).unwrap();
        let dec = minicbor::decode::<SubmitTransactionArgs>(&enc);
        assert!(dec.is_err());
        assert_eq!(
            dec.unwrap_err().to_string(),
            "decode error: Data size over limit"
        );
    }
}
