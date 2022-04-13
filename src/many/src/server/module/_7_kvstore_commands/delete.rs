use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct DeleteArgs {
    #[n(0)]
    pub key: ByteVec,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct DeleteReturns {}
