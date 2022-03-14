use crate::types::ledger;
use minicbor::bytes::ByteVec;
use minicbor::encode::{Error, Write};
use minicbor::{decode, Decode, Decoder, Encode, Encoder};
use std::collections::BTreeMap;

pub struct InfoArgs;
impl<'de> Decode<'de> for InfoArgs {
    fn decode(_d: &mut Decoder<'de>) -> Result<Self, decode::Error> {
        Ok(Self)
    }
}
impl Encode for InfoArgs {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.null()?.ok()
    }
}

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct InfoReturns {
    #[n(0)]
    pub symbols: Vec<ledger::Symbol>,

    #[n(1)]
    pub hash: ByteVec,

    // TODO: this.
    // #[n(2)]
    // pub fees: BTreeMap<Symbol, TransactionFee>,
    // #[n(3)]
    // pub conversion: BTreeMap<(Symbol, Symbol),>,
    //
    /// The list of local names for the symbol. If a symbol is missing from
    /// this map, it may not have a local name but can still be a valid
    /// symbol (refer to the list of symbols above).
    #[n(4)]
    pub local_names: BTreeMap<ledger::Symbol, String>,
}
