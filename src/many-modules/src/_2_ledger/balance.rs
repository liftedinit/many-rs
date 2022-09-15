use many_identity::Address;
use many_types::{ledger, VecOrSingle};
use minicbor::{Decode, Encode};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct BalanceArgs {
    #[n(0)]
    pub account: Option<Address>,

    #[n(1)]
    pub symbols: Option<VecOrSingle<ledger::Symbol>>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct BalanceReturns {
    #[n(0)]
    pub balances: BTreeMap<ledger::Symbol, ledger::TokenAmount>,
}
