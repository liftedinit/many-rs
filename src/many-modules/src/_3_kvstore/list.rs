use crate::kvstore::KeyFilterType;
use many_types::SortOrder;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct ListArgs {
    #[n(0)]
    pub count: Option<u64>,

    #[n(1)]
    pub order: Option<SortOrder>,

    #[n(2)]
    pub filter: Option<Vec<KeyFilterType>>,
}

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct ListReturns {
    #[n(0)]
    pub keys: Vec<ByteVec>,
}
