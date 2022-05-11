
use minicbor::{Decode, Encode};

use crate::{Identity};

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetArgs {
    #[n(0)]
    words: Vec<String>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    identity: Identity
}
