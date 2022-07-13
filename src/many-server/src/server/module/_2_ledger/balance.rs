use crate::types::{ledger, VecOrSingle};
use crate::Address;
use minicbor::{Decode, Encode};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
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
