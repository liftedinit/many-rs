use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Encode, Decode)]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[cbor(map)]
pub struct GetArgs {
    #[n(0)]
    pub key: ByteVec,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    pub value: Option<ByteVec>,
}
