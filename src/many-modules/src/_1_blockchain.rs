use many_error::{define_attribute_many_error, ManyError};
use many_macros::many_module;
use many_types::blockchain::{
    Block, BlockIdentifier, RangeBlockQuery, SingleBlockQuery, SingleTransactionQuery, Transaction,
};
use minicbor::{Decode, Encode};

use many_types::SortOrder;
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

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoReturns {
    #[n(0)]
    pub latest_block: BlockIdentifier,

    #[cbor(n(1), with = "minicbor::bytes")]
    pub app_hash: Option<Vec<u8>>,

    #[n(2)]
    pub retained_height: Option<u64>,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct BlockArgs {
    #[n(0)]
    pub query: SingleBlockQuery,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct BlockReturns {
    #[n(0)]
    pub block: Block,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct TransactionArgs {
    #[n(0)]
    pub query: SingleTransactionQuery,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct TransactionReturns {
    #[n(0)]
    pub txn: Transaction,
}

#[derive(Clone, Debug, Default, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct ListArgs {
    #[n(0)]
    pub count: Option<u64>,

    #[n(1)]
    pub order: Option<SortOrder>,

    #[n(2)]
    pub filter: Option<RangeBlockQuery>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ListReturns {
    #[n(0)]
    pub height: u64,

    #[n(1)]
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct RequestArgs {
    #[n(0)]
    pub query: SingleTransactionQuery,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct RequestReturns {
    #[cbor(n(0), with = "minicbor::bytes")]
    pub request: Vec<u8>,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct ResponseArgs {
    #[n(0)]
    pub query: SingleTransactionQuery,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ResponseReturns {
    #[cbor(n(0), with = "minicbor::bytes")]
    pub response: Vec<u8>,
}

#[many_module(name = BlockchainModule, id = 1, namespace = blockchain, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait BlockchainModuleBackend: Send {
    fn info(&self) -> Result<InfoReturns, ManyError>;
    fn block(&self, args: BlockArgs) -> Result<BlockReturns, ManyError>;
    fn transaction(&self, args: TransactionArgs) -> Result<TransactionReturns, ManyError>;
    fn list(&self, args: ListArgs) -> Result<ListReturns, ManyError>;
    fn request(&self, args: RequestArgs) -> Result<RequestReturns, ManyError>;
    fn response(&self, args: ResponseArgs) -> Result<ResponseReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::{call_module, call_module_cbor};
    use many_types::blockchain::TransactionIdentifier;
    use many_types::{CborRange, Timestamp};
    use mockall::predicate;
    use std::sync::{Arc, Mutex};

    #[test]
    fn info() {
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_info().times(1).return_const(Ok(InfoReturns {
            latest_block: {
                BlockIdentifier {
                    hash: vec![0u8; 8],
                    height: 0,
                }
            },
            app_hash: Some(vec![2u8; 8]),
            retained_height: Some(2),
        }));
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
        let data = BlockArgs {
            query: SingleBlockQuery::Hash(vec![5u8; 8]),
        };
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_block()
            .with(predicate::eq(data.clone()))
            .times(1)
            .returning(|args| match args.query {
                SingleBlockQuery::Hash(v) => Ok(BlockReturns {
                    block: Block {
                        id: BlockIdentifier::new(v, 1),
                        parent: BlockIdentifier::genesis(),
                        app_hash: Some(vec![4u8; 8]),
                        timestamp: Timestamp::now(),
                        txs_count: 1,
                        txs: vec![Transaction {
                            id: TransactionIdentifier { hash: vec![] },
                            request: None,
                            response: None,
                        }],
                    },
                }),
                _ => unimplemented!(),
            });
        let module = super::BlockchainModule::new(Arc::new(Mutex::new(mock)));

        let block_returns: BlockReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "blockchain.block",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            block_returns.block.id,
            BlockIdentifier::new(vec![5u8; 8], 1)
        );
    }

    #[test]
    fn block_height() {
        let data = BlockArgs {
            query: SingleBlockQuery::Height(3),
        };
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_block()
            .with(predicate::eq(data.clone()))
            .times(1)
            .returning(|args| match args.query {
                SingleBlockQuery::Height(h) => Ok(BlockReturns {
                    block: Block {
                        id: BlockIdentifier::new(vec![3u8; 8], h),
                        parent: BlockIdentifier::genesis(),
                        app_hash: Some(vec![4u8; 8]),
                        timestamp: Timestamp::now(),
                        txs_count: 1,
                        txs: vec![Transaction {
                            id: TransactionIdentifier { hash: vec![] },
                            request: None,
                            response: None,
                        }],
                    },
                }),
                _ => unimplemented!(),
            });
        let module = super::BlockchainModule::new(Arc::new(Mutex::new(mock)));

        let block_returns: BlockReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "blockchain.block",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            block_returns.block.id,
            BlockIdentifier::new(vec![3u8; 8], 3)
        );
    }

    #[test]
    fn transaction() {
        let data = TransactionArgs {
            query: SingleTransactionQuery::Hash(vec![6u8; 8]),
        };
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_transaction()
            .with(predicate::eq(data.clone()))
            .times(1)
            .returning(|args| match args.query {
                SingleTransactionQuery::Hash(v) => Ok(TransactionReturns {
                    txn: Transaction {
                        id: TransactionIdentifier { hash: v },
                        request: None,
                        response: None,
                    },
                }),
            });
        let module = super::BlockchainModule::new(Arc::new(Mutex::new(mock)));

        let transaction_returns: TransactionReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "blockchain.transaction",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(transaction_returns.txn.id.hash, vec![6u8; 8]);
        assert_eq!(transaction_returns.txn.request, None);
        assert_eq!(transaction_returns.txn.response, None);
    }

    #[test]
    fn list() {
        let data = ListArgs {
            count: None,
            order: None,
            filter: Some(RangeBlockQuery::Height(CborRange::default())),
        };
        let blocks = vec![Block {
            id: BlockIdentifier {
                hash: vec![1, 0],
                height: 1,
            },
            parent: BlockIdentifier {
                hash: vec![],
                height: 0,
            },
            app_hash: Some(vec![2, 3, 4]),
            timestamp: Timestamp::now(),
            txs_count: 0,
            txs: vec![],
        }];
        let blocks2 = blocks.clone();
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_list()
            .with(predicate::eq(data.clone()))
            .times(1)
            .returning(move |args| match args.filter {
                Some(RangeBlockQuery::Height(_)) => Ok(ListReturns {
                    height: 1,
                    blocks: blocks.clone(),
                }),
                _ => unimplemented!(),
            });
        let module = super::BlockchainModule::new(Arc::new(Mutex::new(mock)));

        let list_returns: ListReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "blockchain.list",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(list_returns.height, 1);
        assert_eq!(list_returns.blocks, blocks2);
    }

    #[test]
    fn request() {
        let data = RequestArgs {
            query: SingleTransactionQuery::Hash(vec![0, 1, 2, 3]),
        };
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_request()
            .with(predicate::eq(data.clone()))
            .times(1)
            .returning(|_args| {
                Ok(RequestReturns {
                    request: vec![3, 2, 1, 0],
                })
            });
        let module = super::BlockchainModule::new(Arc::new(Mutex::new(mock)));

        let request_returns: RequestReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "blockchain.request",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(request_returns.request, vec![3, 2, 1, 0]);
    }

    #[test]
    fn response() {
        let data = ResponseArgs {
            query: SingleTransactionQuery::Hash(vec![6, 7, 8, 9]),
        };
        let mut mock = MockBlockchainModuleBackend::new();
        mock.expect_response()
            .with(predicate::eq(data.clone()))
            .times(1)
            .returning(|_args| {
                Ok(ResponseReturns {
                    response: vec![9, 8, 7, 6],
                })
            });
        let module = super::BlockchainModule::new(Arc::new(Mutex::new(mock)));

        let response_returns: ResponseReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "blockchain.response",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(response_returns.response, vec![9, 8, 7, 6]);
    }
}
