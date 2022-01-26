use minicbor::{decode, Decode, Decoder, Encode};

pub struct TransactionsArgs;
impl<'de> Decode<'de> for TransactionsArgs {
    fn decode(_d: &mut Decoder<'de>) -> Result<Self, decode::Error> {
        Ok(Self)
    }
}

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct TransactionsReturns {
    #[n(0)]
    pub nb_transactions: u64,
}
