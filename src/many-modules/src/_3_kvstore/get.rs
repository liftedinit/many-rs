use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct GetArgs {
    #[n(0)]
    pub key: ByteVec,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    pub value: Option<ByteVec>,
}
