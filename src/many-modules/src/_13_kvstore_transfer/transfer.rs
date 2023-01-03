use crate::EmptyReturn;
use many_identity::Address;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct TransferArgs {
    #[n(0)]
    pub key: ByteVec,

    #[n(1)]
    pub alternative_owner: Option<Address>,

    #[n(2)]
    pub new_owner: Address,
}

pub type TransferReturn = EmptyReturn;
