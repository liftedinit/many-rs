use crate::types::blockchain::{
    Block, BlockIdentifier, SingleBlockQuery, SingleTransactionQuery, Transaction,
};
use crate::{define_attribute_many_error, ManyError};
use many_macros::many_module;
use minicbor::{Decode, Encode};

define_attribute_many_error!(
    attribute 1 => {
        1: pub fn height_out_of_bound(height, min, max)
            => "Height {height} is out of bound. Range: {min} - {max}.",
        2: pub fn invalid_hash() => "Requested hash does not have the right format.",
        3: pub fn unknown_block() => "Requested block query does not match any block.",
        4: pub fn unknown_transaction()
            => "Requested transaction query does not match any transaction.",
    }
);

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct InfoReturns {
    #[n(0)]
    pub latest_block: BlockIdentifier,

    #[cbor(n(1), with = "minicbor::bytes")]
    pub app_hash: Option<Vec<u8>>,

    #[n(2)]
    pub retained_height: Option<u64>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BlockArgs {
    #[n(0)]
    pub query: SingleBlockQuery,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BlockReturns {
    #[n(0)]
    pub block: Block,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct TransactionArgs {
    #[n(0)]
    pub query: SingleTransactionQuery,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct TransactionReturns {
    #[n(0)]
    pub txn: Transaction,
}

#[many_module(name = BlockchainModule, id = 1, namespace = blockchain, many_crate = crate)]
pub trait BlockchainModuleBackend: Send {
    fn info(&self) -> Result<InfoReturns, ManyError>;
    fn block(&self, args: BlockArgs) -> Result<BlockReturns, ManyError>;
    fn transaction(&self, args: TransactionArgs) -> Result<TransactionReturns, ManyError>;
}
