use minicbor::{bytes::ByteVec, Decode, Encode};

pub type RecallPhrase = Vec<String>;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(transparent)]
pub struct CredentialId(#[n(0)] pub ByteVec);

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(transparent)]
pub struct PublicKey(#[n(0)] pub ByteVec);
