use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

use crate::server::module::EmptyReturn;

#[derive(Clone, Encode, Decode)]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[cbor(map)]
pub struct PutArgs {
    #[n(0)]
    pub key: ByteVec,

    #[n(1)]
    pub value: ByteVec,
}

pub type PutReturn = EmptyReturn;