use minicbor::encode::{Error, Write};
use minicbor::{decode, Decode, Decoder, Encode, Encoder};

pub struct TransactionsArgs;
impl<'de> Decode<'de> for TransactionsArgs {
    fn decode(_d: &mut Decoder<'de>) -> Result<Self, decode::Error> {
        Ok(Self)
    }
}

impl Encode for TransactionsArgs {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.null()?.ok()
    }
}

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct TransactionsReturns {
    #[n(0)]
    pub nb_transactions: u64,
}
