use crate::types::TokenAmount;
use crate::Identity;
use minicbor::{Decode, Encode};

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BurnArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub amount: TokenAmount,

    #[n(2)]
    pub symbol: Identity,
}
