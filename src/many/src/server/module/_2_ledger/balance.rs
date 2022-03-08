use crate::types::{ledger, VecOrSingle};
use crate::Identity;
use minicbor::{Decode, Encode};
use std::collections::BTreeMap;

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BalanceArgs {
    #[n(0)]
    pub account: Option<Identity>,

    #[n(1)]
    pub symbols: Option<VecOrSingle<ledger::Symbol>>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BalanceReturns {
    #[n(0)]
    pub balances: BTreeMap<ledger::Symbol, ledger::TokenAmount>,
}
