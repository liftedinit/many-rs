use crate::types::{Symbol, TokenAmount};
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
    pub amount: TokenAmount,

    #[n(3)]
    pub symbol: Symbol,
}
