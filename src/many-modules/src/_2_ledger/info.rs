use crate::EmptyArg;
use many_types::ledger;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};
use std::collections::BTreeMap;

pub type InfoArgs = EmptyArg;

#[derive(Clone, Debug, Decode, Encode)]
#[cbor(map)]
pub struct InfoReturns {
    // TODO: remove this. Kept for backward compatibility.
    #[n(0)]
    pub symbols: Vec<ledger::Symbol>,

    #[n(1)]
    pub hash: ByteVec,

    /// The list of local names for the symbol. If a symbol is missing from
    /// this map, it may not have a local name but can still be a valid
    /// symbol (refer to the list of symbols above).
    // TODO: remove this. Kept for backward compatibility.
    #[n(4)]
    pub local_names: BTreeMap<ledger::Symbol, String>,

    #[n(5)]
    pub tokens: BTreeMap<ledger::Symbol, ledger::TokenInfoSummary>,
}
