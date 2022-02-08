use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct PutArgs {
    #[n(0)]
    pub key: ByteVec,

    #[n(1)]
    pub value: ByteVec,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct PutReturns {}
