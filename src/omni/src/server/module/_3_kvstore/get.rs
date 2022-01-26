use minicbor::{Decode, Encode};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct GetArgs {
    #[n(0)]
    pub key: Vec<u8>,

    #[n(1)]
    pub proof: Option<bool>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    pub value: Option<Vec<u8>>,
}
