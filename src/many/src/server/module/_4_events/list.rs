use crate::types::{events, SortOrder, TransactionFilter};
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct ListArgs {
    #[n(0)]
    pub count: Option<u64>,

    #[n(1)]
    pub order: Option<SortOrder>,

    #[n(2)]
    pub filter: Option<TransactionFilter>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct ListReturns {
    #[n(0)]
    pub nb_events: u64,

    #[n(1)]
    pub events: Vec<events::Transaction>,
}
