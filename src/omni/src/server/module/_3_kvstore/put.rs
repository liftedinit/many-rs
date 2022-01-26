use minicbor::{Decode, Encode};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct PutArgs {
    #[n(0)]
    pub key: Vec<u8>,

    #[n(1)]
    pub value: Vec<u8>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct PutReturns {}
