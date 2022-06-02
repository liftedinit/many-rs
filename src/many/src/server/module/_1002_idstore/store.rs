use super::types::{CredentialId, PublicKey, RecallPhrase};
use crate::Identity;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct StoreArgs {
    #[n(0)]
    pub address: Identity,

    #[n(1)]
    pub cred_id: CredentialId,

    #[n(2)]
    pub public_key: PublicKey,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct StoreReturns(#[n(0)] pub RecallPhrase);
