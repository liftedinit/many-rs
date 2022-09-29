use crate::EmptyArg;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

pub type InfoArg = EmptyArg;

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct InfoReturns {
    #[n(0)]
    pub hash: ByteVec,
}
