use crate::types::Timestamp;
use minicbor::encode::{Error, Write};
use minicbor::{decode, Decode, Decoder, Encode, Encoder};

pub enum SingleBlockQuery {
    Hash(Vec<u8>),
    Height(u64),
}

impl Encode for SingleBlockQuery {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        match &self {
            SingleBlockQuery::Hash(hash) => {
                e.map(1)?.u8(0)?.bytes(hash)?;
            }
            SingleBlockQuery::Height(height) => {
                e.map(1)?.u8(1)?.u64(*height)?;
            }
        }
        Ok(())
    }
}

impl<'d> Decode<'d> for SingleBlockQuery {
    fn decode(d: &mut Decoder<'d>) -> Result<Self, decode::Error> {
        let mut indefinite = false;
        let key = match d.map()? {
            None => {
                indefinite = true;
                d.u8()
            }
            Some(1) => d.u8(),
            Some(_) => Err(decode::Error::Message(
                "Invalid length for single block query map.",
            )),
        }?;

        let result = match key {
            0 => Ok(SingleBlockQuery::Hash(d.bytes()?.to_vec().into())),
            1 => Ok(SingleBlockQuery::Height(d.u64()?)),
            x => Err(decode::Error::UnknownVariant(x as u32)),
        };

        if indefinite {
            d.skip()?;
        }

        result
    }
}

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct BlockIdentifier {
    #[cbor(n(0), with = "minicbor::bytes")]
    pub hash: Vec<u8>,

    #[n(1)]
    pub height: u64,
}

impl BlockIdentifier {
    pub fn new(hash: Vec<u8>, height: u64) -> Self {
        Self { hash, height }
    }

    pub fn genesis() -> Self {
        Self::new(vec![], 0)
    }
}

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct TransactionIdentifier {
    #[cbor(n(0), with = "minicbor::bytes")]
    pub hash: Vec<u8>,
}

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct Transaction {
    #[n(0)]
    pub id: TransactionIdentifier,

    #[cbor(n(1), with = "minicbor::bytes")]
    pub content: Option<Vec<u8>>,
}

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct Block {
    #[n(0)]
    pub id: BlockIdentifier,

    #[n(1)]
    pub parent: BlockIdentifier,

    #[n(2)]
    pub timestamp: Timestamp,

    #[n(3)]
    pub txs_count: u64,

    #[n(4)]
    pub txs: Vec<Transaction>,
}
