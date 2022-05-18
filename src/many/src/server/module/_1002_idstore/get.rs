use super::{types::{CredentialId, RecallPhrase}, PublicKey};
use crate::Identity;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode)]
#[cfg_attr(test, derive(PartialEq))]
#[cbor(transparent)]
pub struct GetFromRecallPhraseArgs(#[n(0)] pub RecallPhrase);

#[derive(Clone, Debug, Encode, Decode)]
#[cfg_attr(test, derive(PartialEq))]
#[cbor(transparent)]
pub struct GetFromAddressArgs(#[n(0)] pub Identity);

#[derive(Clone, Encode, Decode)]
pub struct GetReturns{
    #[n(0)]
    pub cred_id: CredentialId,

    #[n(1)]
    pub public_key: PublicKey,
}
