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

#[cfg(test)]
mod tests {
    use crate::server::module::testutils::{call_module_cbor, call_module_cbor_diag};
    use crate::types::{blockchain::TransactionIdentifier, Timestamp};
    use std::sync::{Arc, Mutex};

    use super::*;
    use proptest::prelude::*;

    #[derive(Default)]
    struct BlockchainImpl;

    impl super::BlockchainModuleBackend for BlockchainImpl {
        fn info(&self) -> Result<InfoReturns, ManyError> {
            Ok(InfoReturns {
                latest_block: BlockIdentifier::new(vec![0u8; 8], 0),
                app_hash: None,
                retained_height: None,
            })
        }

        fn block(&self, args: BlockArgs) -> Result<BlockReturns, ManyError> {
            match args.query {
                SingleBlockQuery::Hash(v) => Ok(BlockReturns {
                    block: Block {
                        id: BlockIdentifier::new(v, 1),
                        parent: BlockIdentifier::genesis(),
                        post_hash: None,
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
                        id: BlockIdentifier::new(vec![1u8; 8], h),
                        parent: BlockIdentifier::genesis(),
                        post_hash: None,
                        timestamp: Timestamp::now(),
                        txs_count: 1,
                        txs: vec![Transaction {
                            id: TransactionIdentifier { hash: vec![] },
                            content: None,
                        }],
                    },
                }),
            }
        }

        fn transaction(&self, args: TransactionArgs) -> Result<TransactionReturns, ManyError> {
            match args.query {
                SingleTransactionQuery::Hash(v) => Ok(TransactionReturns {
                    txn: Transaction {
                        id: TransactionIdentifier { hash: v },
                        content: None,
                    },
                }),
            }
        }
    }

    prop_compose! {
        fn arb_hash()(v in proptest::collection::vec(any::<u8>(), 1..32)) -> Vec<u8> {
            v
        }
    }

    #[test]
    fn info() {
        let module_impl = Arc::new(Mutex::new(BlockchainImpl::default()));
        let module = super::BlockchainModule::new(module_impl);

        let info_returns: InfoReturns =
            minicbor::decode(&call_module_cbor_diag(&module, "blockchain.info", "null").unwrap())
                .unwrap();

        assert_eq!(
            info_returns.latest_block,
            BlockIdentifier::new(vec![0u8; 8], 0)
        );
        assert_eq!(info_returns.app_hash, None);
        assert_eq!(info_returns.retained_height, None);
    }

    proptest! {
        #[test]
        fn block_hash(v in arb_hash()) {
            let module_impl = Arc::new(Mutex::new(BlockchainImpl::default()));
            let module = super::BlockchainModule::new(module_impl);

            let data = BlockArgs {
                query: SingleBlockQuery::Hash(v.clone())
            };
            let data = minicbor::to_vec(data).unwrap();

            let block_returns: BlockReturns = minicbor::decode(
                &call_module_cbor(
                    &module,
                    "blockchain.block",
                    data
                )
                .unwrap(),
            )
            .unwrap();

            assert_eq!(
                block_returns.block.id,
                BlockIdentifier::new(v, 1)
            );
        }

        #[test]
        fn block_height(h in any::<u64>()) {
            let module_impl = Arc::new(Mutex::new(BlockchainImpl::default()));
            let module = super::BlockchainModule::new(module_impl);

            let data = BlockArgs {
                query: SingleBlockQuery::Height(h)
            };
            let data = minicbor::to_vec(data).unwrap();

            let block_returns: BlockReturns = minicbor::decode(
                &call_module_cbor(&module, "blockchain.block", data).unwrap(),
            )
            .unwrap();

            assert_eq!(
                block_returns.block.id,
                BlockIdentifier::new(vec![1u8; 8], h)
            );
        }

        #[test]
        fn transaction(v in arb_hash()) {
            let module_impl = Arc::new(Mutex::new(BlockchainImpl::default()));
            let module = super::BlockchainModule::new(module_impl);

            let data = TransactionArgs {
                query: SingleTransactionQuery::Hash(v.clone())
            };
            let data = minicbor::to_vec(data).unwrap();

            let transaction_returns: TransactionReturns = minicbor::decode(
                &call_module_cbor(
                    &module,
                    "blockchain.transaction",
                    data
                )
                .unwrap(),
            )
            .unwrap();

            assert_eq!(transaction_returns.txn.id.hash, v);
            assert_eq!(transaction_returns.txn.content, None);
        }
    }
}
