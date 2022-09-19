use crate::Timestamp;
use minicbor::encode::{Error, Write};
use minicbor::{decode, Decode, Decoder, Encode, Encoder};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SingleBlockQuery {
    Hash(Vec<u8>),
    Height(u64),
}

impl<C> Encode<C> for SingleBlockQuery {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
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

impl<'d, C> Decode<'d, C> for SingleBlockQuery {
    fn decode(d: &mut Decoder<'d>, _: &mut C) -> Result<Self, decode::Error> {
        let mut indefinite = false;
        let key = match d.map()? {
            None => {
                indefinite = true;
                d.u8()
            }
            Some(1) => d.u8(),
            Some(_) => Err(decode::Error::message(
                "Invalid length for single block query map.",
            )),
        }?;

        let result = match key {
            0 => Ok(SingleBlockQuery::Hash(d.bytes()?.to_vec())),
            1 => Ok(SingleBlockQuery::Height(d.u64()?)),
            x => Err(decode::Error::unknown_variant(u32::from(x))),
        };

        if indefinite {
            d.skip()?;
        }

        result
    }
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
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

#[derive(Debug, Clone, Decode, Encode)]
#[cbor(map)]
pub struct TransactionIdentifier {
    #[cbor(n(0), with = "minicbor::bytes")]
    pub hash: Vec<u8>,
}

#[derive(Debug, Clone, Decode, Encode)]
#[cbor(map)]
pub struct Transaction {
    #[n(0)]
    pub id: TransactionIdentifier,

    #[cbor(n(1), with = "minicbor::bytes")]
    pub content: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Decode, Encode)]
#[cbor(map)]
pub struct Block {
    #[n(0)]
    pub id: BlockIdentifier,

    #[n(1)]
    pub parent: BlockIdentifier,

    #[n(2)]
    pub app_hash: Option<Vec<u8>>,

    #[n(3)]
    pub timestamp: Timestamp,

    #[n(4)]
    pub txs_count: u64,

    #[n(5)]
    pub txs: Vec<Transaction>,
}

// TODO: This doesn't look right according to the spec
// single-transaction-query =
//     ; A transaction hash.
//     { 0 => bstr }
//     ; A block + transaction index.
//     / { 1 => [ single-block-query, uint ] }
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SingleTransactionQuery {
    Hash(Vec<u8>),
}

impl<C> Encode<C> for SingleTransactionQuery {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        match &self {
            SingleTransactionQuery::Hash(hash) => {
                e.map(1)?.u8(0)?.bytes(hash)?;
            }
        }
        Ok(())
    }
}

impl<'d, C> Decode<'d, C> for SingleTransactionQuery {
    fn decode(d: &mut Decoder<'d>, _: &mut C) -> Result<Self, decode::Error> {
        let mut indefinite = false;
        let key = match d.map()? {
            None => {
                indefinite = true;
                d.u8()
            }
            Some(1) => d.u8(),
            Some(_) => Err(decode::Error::message(
                "Invalid hash for single transaction query.",
            )),
        }?;

        let result = match key {
            0 => Ok(SingleTransactionQuery::Hash(d.bytes()?.to_vec())),
            x => Err(decode::Error::unknown_variant(u32::from(x))),
        };

        if indefinite {
            d.skip()?;
        }

        result
    }
}
