use crate::account::features::{Feature, FeatureId, TryCreateFeature};
use crate::account::Role;
use crate::events::{AccountMultisigTransaction, AddressContainer};
use crate::ledger::SendArgs;
use crate::EmptyReturn;
use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;
use many_protocol::ResponseMessage;
use many_types::cbor::CborAny;
use many_types::ledger::TokenAmount;
use many_types::legacy::{DataLegacy, MemoLegacy};
use many_types::{legacy, Memo, Timestamp};
use minicbor::bytes::ByteVec;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use std::collections::{BTreeMap, BTreeSet};

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
    pub memo_: Option<MemoLegacy<String>>,

    #[n(2)]
    pub transaction: Box<AccountMultisigTransaction>,

    #[n(3)]
    pub threshold: Option<u64>,

    #[n(4)]
    pub timeout_in_secs: Option<u64>,

    #[n(5)]
    pub execute_automatically: Option<bool>,

    #[n(6)]
    pub data_: Option<DataLegacy>,

    #[n(7)]
    pub memo: Option<Memo>,
}

impl SubmitTransactionArgs {
    pub fn send(
        from: Address,
        to: Address,
        symbol: Address,
        amount: TokenAmount,
        memo: Option<Memo>,
    ) -> Self {
        Self {
            account: from,
            transaction: Box::new(AccountMultisigTransaction::Send(SendArgs {
                from: Some(from),
                to,
                symbol,
                amount,
                memo,
            })),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
            memo_: None,
            data_: None,
            memo: None,
        }
    }
}

impl AddressContainer for SubmitTransactionArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        let mut set = BTreeSet::from([self.account]);
        set.extend(self.transaction.addresses());
        set
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
    pub memo_: Option<legacy::MemoLegacy<String>>,

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
    pub data_: Option<DataLegacy>,

    #[n(8)]
    pub state: MultisigTransactionState,

    #[n(9)]
    pub memo: Option<Memo>,
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

impl AddressContainer for SetDefaultsArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        BTreeSet::from([self.account])
    }
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
