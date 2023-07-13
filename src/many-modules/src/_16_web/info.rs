use minicbor::{Decode, Encode};
use minicbor::bytes::ByteVec;
use crate::EmptyArg;

pub type InfoArg = EmptyArg;

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct InfoReturns {
    #[n(0)]
    pub hash: ByteVec,
}