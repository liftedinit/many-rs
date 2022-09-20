use crate::EmptyReturn;
use many_error::Reason;
use many_identity::Address;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct DisableArgs {
    #[n(0)]
    pub key: ByteVec,

    #[n(1)]
    pub alternative_owner: Option<Address>,

    #[n(2)]
    pub reason: Option<Reason<u64>>,
}

pub type DisableReturn = EmptyReturn;
