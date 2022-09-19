use super::{
    types::{CredentialId, RecallPhrase},
    PublicKey,
};
use many_identity::Address;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct GetFromRecallPhraseArgs(#[n(0)] pub RecallPhrase);

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct GetFromAddressArgs(#[n(0)] pub Address);

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    pub cred_id: CredentialId,

    #[n(1)]
    pub public_key: PublicKey,
}
