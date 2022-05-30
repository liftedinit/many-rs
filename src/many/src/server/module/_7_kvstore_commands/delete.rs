use crate::server::module::EmptyReturn;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Encode, Decode)]
#[cfg_attr(test, derive(Debug, PartialEq))]
#[cbor(map)]
pub struct DeleteArgs {
    #[n(0)]
    pub key: ByteVec,
}

pub type DeleteReturn = EmptyReturn;
