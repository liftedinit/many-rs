use super::types::{CredentialId, RecallPhrase};
use crate::Identity;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode)]
#[cfg_attr(test, derive(PartialEq))]
#[cbor(map)]
pub struct StoreArgs {
    #[n(0)]
    pub address: Identity,

    #[n(1)]
    pub cred_id: CredentialId,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(transparent)]
pub struct StoreReturns(#[n(0)] pub RecallPhrase);
