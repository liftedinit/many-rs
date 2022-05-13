use minicbor::{Encode, Decode};

pub type RecallPhrase = Vec<String>; // TODO: Change this?

#[derive(Clone, Encode, Decode)]
#[cbor(transparent)]
pub struct CredentialId(#[n(0)] u16);

impl From<Vec<u8>> for CredentialId {
    fn from(v: Vec<u8>) -> CredentialId {
        let n = ((v[0] as u16) << 8) | v[1] as u16;
        CredentialId(n)
    }
}

impl From<CredentialId> for Vec<u8> {
    fn from(c: CredentialId) -> Vec<u8> {
        c.0.to_be_bytes().to_vec()
    }
}