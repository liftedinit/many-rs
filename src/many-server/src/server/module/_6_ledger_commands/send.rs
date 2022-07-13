use crate::Address;
use crate::{server::module::EmptyReturn, types::ledger};
use minicbor::{Decode, Encode};

#[derive(Debug, Clone, Encode, Decode, PartialEq)]
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
}

pub type SendReturns = EmptyReturn;
