
use minicbor::{Decode, Encode};

use crate::{Identity};

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetArgs {
    #[n(0)]
    pub words: Vec<String>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    pub identity: Identity
}
