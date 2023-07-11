use crate::EmptyReturn;
use minicbor::{Decode, Encode};

pub type CloseReturns = EmptyReturn;

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct CloseArgs {
    #[n(0)]
    pub dseq: u64,
}
