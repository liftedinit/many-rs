use minicbor::{Decode, Encode};
use crate::{Identity};
use super::types::{RecallPhrase, CredentialId};

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetFromRecallPhraseArgs {
    #[n(0)]
    pub recall_phrase: RecallPhrase,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetFromAddressArgs {
    #[n(0)]
    pub address: Identity
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    pub cred_id: CredentialId
}
