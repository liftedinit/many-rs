use crate::events;
use many_types::SortOrder;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Default, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct ListArgs {
    #[n(0)]
    pub count: Option<u64>,

    #[n(1)]
    pub order: Option<SortOrder>,

    #[n(2)]
    pub filter: Option<events::EventFilter>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct ListReturns {
    #[n(0)]
    pub nb_events: u64,

    #[n(1)]
    pub events: Vec<events::EventLog>,
}
