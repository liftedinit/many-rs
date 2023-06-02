use many_types::SortOrder;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct ListArgs {
    #[n(0)]
    pub order: Option<SortOrder>,
}

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct ListReturns {
    #[n(0)]
    pub keys: Vec<ByteVec>,
}
