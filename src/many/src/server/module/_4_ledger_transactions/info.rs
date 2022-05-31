use crate::server::module::EmptyArg;
use minicbor::{Decode, Encode};

pub type TransactionsArgs = EmptyArg;

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct TransactionsReturns {
    #[n(0)]
    pub nb_transactions: u64,
}
