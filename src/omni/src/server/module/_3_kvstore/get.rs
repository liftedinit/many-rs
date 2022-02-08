use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct GetArgs {
    #[n(0)]
    pub key: ByteVec,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    pub value: Option<ByteVec>,
}
