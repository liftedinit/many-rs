use crate::EmptyReturn;

pub type CloseReturns = EmptyReturn;

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct CloseArgs {
    #[n(0)]
    pub dseq: u64,
}
