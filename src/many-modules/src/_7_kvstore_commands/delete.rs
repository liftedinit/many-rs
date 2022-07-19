use crate::EmptyReturn;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct DeleteArgs {
    #[n(0)]
    pub key: ByteVec,
}

pub type DeleteReturn = EmptyReturn;
