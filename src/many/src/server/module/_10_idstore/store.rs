use minicbor::{Decode, Encode};

use crate::{Identity, server::module::EmptyReturn};

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct StoreArgs {
    #[n(0)]
    pub words: Vec<String>,

    #[n(1)]
    pub identity: Identity,
}

pub type StoreReturn = EmptyReturn;