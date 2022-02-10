use crate::types::ledger;
use crate::Identity;
use minicbor::{Decode, Encode};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct SendArgs {
    #[n(0)]
    pub from: Option<Identity>,

    #[n(1)]
    pub to: Identity,

    #[n(2)]
    pub amount: ledger::TokenAmount,

    #[n(3)]
    pub symbol: ledger::Symbol,
}
