
use minicbor::{Decode, Encode};

use crate::{Identity};

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetFromRecallPhraseArgs {
    #[n(0)]
    pub words: Vec<String>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetFromAddressArgs {
    #[n(0)]
    pub id: Identity
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetReturns {
    #[n(0)]
    pub cred_id: Vec<u8>
}
