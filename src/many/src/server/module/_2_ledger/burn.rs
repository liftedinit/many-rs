use crate::types::ledger;
use crate::Identity;
use minicbor::{Decode, Encode};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BurnArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub amount: ledger::TokenAmount,

    #[n(2)]
    pub symbol: Identity,
}
