use crate::cbor::CborAny;
use crate::message::ResponseMessage;
use crate::server::module::account::features::{Feature, FeatureId, TryCreateFeature};
use crate::server::module::EmptyReturn;
use crate::types::ledger::TransactionInfo;
use crate::types::Timestamp;
use crate::{Identity, ManyError};
use many_macros::many_module;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};
use std::collections::BTreeMap;

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct MultisigAccountFeatureArg {
    #[n(0)]
    pub threshold: Option<u64>,

    #[n(1)]
    pub timeout_in_secs: Option<u64>,

    #[n(2)]
    pub execute_automatically: Option<bool>,
}

pub struct MultisigAccountFeature {
    pub arg: MultisigAccountFeatureArg,
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

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct SubmitTransactionArg {
    #[n(0)]
    pub memo: Option<String>,

    #[n(1)]
    pub transaction: TransactionInfo,

    #[n(2)]
    pub threshold: Option<u64>,

    #[n(3)]
    pub timeout_in_secs: Option<u64>,

    #[n(4)]
    pub execute_automatically: Option<bool>,
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

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ApproverInfo {
    #[n(0)]
    approved: bool,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoReturn {
    #[n(0)]
    pub memo: Option<String>,

    #[n(1)]
    pub transaction: TransactionInfo,

    #[n(2)]
    pub approvers: BTreeMap<Identity, ApproverInfo>,

    #[n(3)]
    pub threshold: Option<u64>,

    #[n(4)]
    pub execute_automatically: Option<bool>,

    #[n(5)]
    pub timeout: Timestamp,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ApproveArg {
    #[n(0)]
    pub token: ByteVec,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct RevokeArg {
    #[n(0)]
    pub token: ByteVec,
}

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

#[many_module(name = AccountMultisigModule, namespace = account, many_crate = crate)]
pub trait AccountMultisigModuleBackend: Send {
    fn multisig_submit_transaction(
        &mut self,
        sender: &Identity,
        args: SubmitTransactionArg,
    ) -> Result<SubmitTransactionReturn, ManyError>;
    fn multisig_info(&self, sender: &Identity, args: InfoArg) -> Result<InfoReturn, ManyError>;
    fn multisig_approve(
        &self,
        sender: &Identity,
        args: ApproveArg,
    ) -> Result<EmptyReturn, ManyError>;
    fn multisig_revoke(&self, sender: &Identity, args: RevokeArg)
        -> Result<EmptyReturn, ManyError>;
    fn multisig_execute(
        &self,
        sender: &Identity,
        args: ExecuteArg,
    ) -> Result<ResponseMessage, ManyError>;
    fn multisig_withdraw(
        &self,
        sender: &Identity,
        args: WithdrawArg,
    ) -> Result<EmptyReturn, ManyError>;
}
