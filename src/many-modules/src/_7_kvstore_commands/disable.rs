use crate::EmptyReturn;
use derive_builder::Builder;
use many_identity::Address;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Builder, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct DisableArgs {
    #[n(0)]
    pub key: ByteVec,

    #[n(1)]
    #[builder(default = "None")]
    pub alternative_owner: Option<Address>,
}

pub type DisableReturn = EmptyReturn;
