use crate::types::{ledger, SortOrder, TransactionFilter};
use minicbor::{Decode, Encode};

#[derive(Encode, Decode)]
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
    pub nb_transactions: u64,

    #[n(1)]
    pub transactions: Vec<ledger::Transaction>,
}
