use crate::cbor::CborAny;
use crate::message::ResponseMessage;
use crate::server::module::account::features::{Feature, FeatureId, TryCreateFeature};
use crate::server::module::EmptyReturn;
use crate::types::ledger::{TokenAmount, TransactionInfo};
use crate::types::Timestamp;
use crate::{Identity, ManyError};
use many_macros::many_module;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};
use std::collections::{BTreeMap, BTreeSet};

pub mod errors {
    use crate::define_attribute_many_error;
    define_attribute_many_error!(
        attribute 9 => {
            100: pub fn transaction_cannot_be_found() => "The transaction cannot be found.",
            101: pub fn user_cannot_approve_transaction() => "The user is not in the list of approvers.",
            102: pub fn transaction_type_unsupported() => "This transaction is not supported.",
            103: pub fn cannot_execute_transaction() => "This transaction cannot be executed yet.",
        }
    );
}

#[derive(Default, Clone, Encode, Decode)]
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

    fn roles() -> BTreeSet<String> {
        BTreeSet::from([
            "canMultisigSubmit".to_string(),
            "canMultisigApprove".to_string(),
        ])
    }
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct SubmitTransactionArg {
    #[n(0)]
    pub account: Option<Identity>,

    #[n(1)]
    pub memo: Option<ByteVec>,

    #[n(2)]
    pub transaction: TransactionInfo,

    #[n(3)]
    pub threshold: Option<u64>,

    #[n(4)]
    pub timeout_in_secs: Option<u64>,

    #[n(5)]
    pub execute_automatically: Option<bool>,
}

impl SubmitTransactionArg {
    pub fn send(from: Identity, to: Identity, symbol: Identity, amount: TokenAmount) -> Self {
        Self {
            account: Some(from),
            memo: None,
            transaction: TransactionInfo::Send {
                from,
                to,
                symbol,
                amount,
            },
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
        }
    }
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct SubmitTransactionReturn {
    #[n(0)]
    pub token: ByteVec,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoArg {
    #[n(0)]
    pub token: ByteVec,
}

#[derive(Clone, Debug, Default, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct ApproverInfo {
    #[n(0)]
    pub approved: bool,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoReturn {
    #[n(0)]
    pub memo: Option<ByteVec>,

    #[n(1)]
    pub transaction: TransactionInfo,

    #[n(2)]
    pub submitter: Identity,

    #[n(3)]
    pub approvers: BTreeMap<Identity, ApproverInfo>,

    #[n(4)]
    pub threshold: u64,

    #[n(5)]
    pub execute_automatically: bool,

    #[n(6)]
    pub timeout: Timestamp,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct SetDefaultsArg {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub threshold: Option<u64>,

    #[n(2)]
    pub timeout_in_secs: Option<u64>,

    #[n(3)]
    pub execute_automatically: Option<bool>,
}

pub type SetDefaultsReturn = EmptyReturn;

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ApproveArg {
    #[n(0)]
    pub token: ByteVec,
}

pub type ApproveReturn = EmptyReturn;

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct RevokeArg {
    #[n(0)]
    pub token: ByteVec,
}

pub type RevokeReturn = EmptyReturn;

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ExecuteArg {
    #[n(0)]
    pub token: ByteVec,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct WithdrawArg {
    #[n(0)]
    pub token: ByteVec,
}

pub type WithdrawReturn = EmptyReturn;

#[many_module(name = AccountMultisigModule, namespace = account, many_crate = crate)]
pub trait AccountMultisigModuleBackend: Send {
    fn multisig_submit_transaction(
        &mut self,
        sender: &Identity,
        args: SubmitTransactionArg,
    ) -> Result<SubmitTransactionReturn, ManyError>;
    fn multisig_info(&self, sender: &Identity, args: InfoArg) -> Result<InfoReturn, ManyError>;
    fn multisig_set_defaults(
        &mut self,
        sender: &Identity,
        args: SetDefaultsArg,
    ) -> Result<SetDefaultsReturn, ManyError>;
    fn multisig_approve(
        &mut self,
        sender: &Identity,
        args: ApproveArg,
    ) -> Result<ApproveReturn, ManyError>;
    fn multisig_revoke(
        &mut self,
        sender: &Identity,
        args: RevokeArg,
    ) -> Result<RevokeReturn, ManyError>;
    fn multisig_execute(
        &mut self,
        sender: &Identity,
        args: ExecuteArg,
    ) -> Result<ResponseMessage, ManyError>;
    fn multisig_withdraw(
        &mut self,
        sender: &Identity,
        args: WithdrawArg,
    ) -> Result<WithdrawReturn, ManyError>;
}
