use crate::types::blockchain::{
    Block, BlockIdentifier, SingleBlockQuery, SingleTransactionQuery, Transaction,
};
use crate::{define_attribute_many_error, ManyError};
use many_macros::many_module;
use minicbor::{Decode, Encode};

#[cfg(test)]
use mockall::{automock, predicate::*};

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
#[cfg_attr(test, automock)]
pub trait BlockchainModuleBackend: Send {
    fn info(&self) -> Result<InfoReturns, ManyError>;
    fn block(&self, args: BlockArgs) -> Result<BlockReturns, ManyError>;
    fn transaction(&self, args: TransactionArgs) -> Result<TransactionReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        server::module::testutils::{call_module, call_module_cbor},
        types::{blockchain::TransactionIdentifier, Timestamp},
    };

    use super::*;
    #[test]
    fn info() {
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_info().times(1).returning(|| {
            Ok(InfoReturns {
                latest_block: {
                    BlockIdentifier {
                        hash: vec![0u8; 8],
                        height: 0,
                    }
                },
                app_hash: Some(vec![2u8; 8]),
                retained_height: Some(2),
            })
        });
        let module = super::BlockchainModule::new(Arc::new(Mutex::new(mock)));

        let info_returns: InfoReturns =
            minicbor::decode(&call_module(1, &module, "blockchain.info", "null").unwrap()).unwrap();

        assert_eq!(
            info_returns.latest_block,
            BlockIdentifier::new(vec![0u8; 8], 0)
        );
        assert_eq!(info_returns.app_hash, Some(vec![2u8; 8]));
        assert_eq!(info_returns.retained_height, Some(2));
    }

    #[test]
    fn block_hash() {
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_block()
            .times(2)
            .returning(|args| match args.query {
                SingleBlockQuery::Hash(v) => Ok(BlockReturns {
                    block: Block {
                        id: BlockIdentifier::new(v, 1),
                        parent: BlockIdentifier::genesis(),
                        post_hash: Some(vec![4u8; 8]),
                        timestamp: Timestamp::now(),
                        txs_count: 1,
                        txs: vec![Transaction {
                            id: TransactionIdentifier { hash: vec![] },
                            content: None,
                        }],
                    },
                }),
                SingleBlockQuery::Height(h) => Ok(BlockReturns {
                    block: Block {
                        id: BlockIdentifier::new(vec![3u8; 8], h),
                        parent: BlockIdentifier::genesis(),
                        post_hash: Some(vec![4u8; 8]),
                        timestamp: Timestamp::now(),
                        txs_count: 1,
                        txs: vec![Transaction {
                            id: TransactionIdentifier { hash: vec![] },
                            content: None,
                        }],
                    },
                }),
            });
        let module = super::BlockchainModule::new(Arc::new(Mutex::new(mock)));

        let data = BlockArgs {
            query: SingleBlockQuery::Hash(vec![5u8; 8]),
        };
        let data = minicbor::to_vec(data).unwrap();

        let block_returns: BlockReturns =
            minicbor::decode(&call_module_cbor(1, &module, "blockchain.block", data).unwrap())
                .unwrap();

        assert_eq!(
            block_returns.block.id,
            BlockIdentifier::new(vec![5u8; 8], 1)
        );

        let data = BlockArgs {
            query: SingleBlockQuery::Height(3),
        };
        let data = minicbor::to_vec(data).unwrap();

        let block_returns: BlockReturns =
            minicbor::decode(&call_module_cbor(1, &module, "blockchain.block", data).unwrap())
                .unwrap();

        assert_eq!(
            block_returns.block.id,
            BlockIdentifier::new(vec![3u8; 8], 3)
        );
    }

    #[test]
    fn transaction() {
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_transaction()
            .times(1)
            .returning(|args| match args.query {
                SingleTransactionQuery::Hash(v) => Ok(TransactionReturns {
                    txn: Transaction {
                        id: TransactionIdentifier { hash: v },
                        content: None,
                    },
                }),
            });
        let module = super::BlockchainModule::new(Arc::new(Mutex::new(mock)));

        let data = TransactionArgs {
            query: SingleTransactionQuery::Hash(vec![6u8; 8]),
        };
        let data = minicbor::to_vec(data).unwrap();

        let transaction_returns: TransactionReturns = minicbor::decode(
            &call_module_cbor(1, &module, "blockchain.transaction", data).unwrap(),
        )
        .unwrap();

        assert_eq!(transaction_returns.txn.id.hash, vec![6u8; 8]);
        assert_eq!(transaction_returns.txn.content, None);
    }
}
