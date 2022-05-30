use minicbor::{Encode, Decode, bytes::ByteVec};

pub type RecallPhrase = Vec<String>;

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
#[cbor(transparent)]
pub struct CredentialId(#[n(0)] pub ByteVec);

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
#[cbor(transparent)]
pub struct PublicKey(#[n(0)] pub ByteVec);