use crate::events::AddressContainer;
use crate::EmptyReturn;
use many_identity::Address;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};
use std::collections::BTreeSet;

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

impl AddressContainer for TransferArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        let mut addresses = self.alternative_owner.addresses();
        addresses.insert(self.new_owner);
        addresses
    }
}

pub type TransferReturn = EmptyReturn;
