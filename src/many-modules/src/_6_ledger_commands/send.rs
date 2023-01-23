use crate::events::AddressContainer;
use crate::EmptyReturn;
use many_identity::Address;
use many_types::{ledger, Memo};
use minicbor::{Decode, Encode};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct SendArgs {
    #[n(0)]
    pub from: Option<Address>,

    #[n(1)]
    pub to: Address,

    #[n(2)]
    pub amount: ledger::TokenAmount,

    #[n(3)]
    pub symbol: ledger::Symbol,

    #[n(4)]
    pub memo: Option<Memo>,
}

pub type SendReturns = EmptyReturn;

impl AddressContainer for SendArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        match self.from {
            Some(from) => BTreeSet::from([from, self.to]),
            None => BTreeSet::from([self.to]),
        }
    }
}
