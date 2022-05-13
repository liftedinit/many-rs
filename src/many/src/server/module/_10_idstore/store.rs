use minicbor::{Decode, Encode};
use crate::{Identity, server::module::EmptyReturn};
use super::types::{RecallPhrase, CredentialId};

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct StoreArgs {
    #[n(0)]
    pub recall_phrase: RecallPhrase,

    #[n(1)]
    pub address: Identity,

    #[n(2)]
    pub cred_id: CredentialId,
}

pub type StoreReturn = EmptyReturn;