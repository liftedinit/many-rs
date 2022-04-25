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
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use crate::{
        message::RequestMessage,
        message::{RequestMessageBuilder, ResponseMessage},
        server::tests::execute_request,
        types::identity::{cose::tests::generate_random_eddsa_identity, CoseKeyIdentity},
        ManyServer,
    };

    use crate::types::{blockchain::TransactionIdentifier, Timestamp};

    use super::*;
    use proptest::prelude::*;

    const SERVER_VERSION: u8 = 1;

    #[derive(Default)]
    struct BlockchainImpl(pub Vec<u64>);

    impl super::BlockchainModuleBackend for BlockchainImpl {
        fn info(&self) -> Result<InfoReturns, ManyError> {
            // TODO: Fix mock
            Ok(InfoReturns {
                latest_block: BlockIdentifier::new(vec![0u8; 8], 0),
                app_hash: None,
                retained_height: None,
            })
        }

        fn block(&self, args: BlockArgs) -> Result<BlockReturns, ManyError> {
            // TODO: Fix mock
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
            // TODO: Fix mock
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
        /// Generate MANY server with arbitrary name composed of arbitrary non-control characters.
        fn arb_server()(name in "\\PC*") -> (CoseKeyIdentity, Arc<Mutex<ManyServer>>) {
            let id = generate_random_eddsa_identity();
            let server = ManyServer::new(name, id.clone());
            let blockchain_impl = Arc::new(Mutex::new(BlockchainImpl::default()));
            let blockchain_module = BlockchainModule::new(blockchain_impl);

            {
                let mut s = server.lock().unwrap();
                s.version = Some(SERVER_VERSION.to_string());
                s.add_module(blockchain_module);
            }

            (id, server)
        }
    }

    // TODO: Refactor using `call_module()` from the Account PR
    proptest! {
        #[test]
        fn info((id, server) in arb_server()) {
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("blockchain.info".to_string())
                .data("null".as_bytes().to_vec())
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let info_returns: InfoReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(info_returns.latest_block, BlockIdentifier::new(vec![0u8; 8], 0));
            assert_eq!(info_returns.app_hash, None);
            assert_eq!(info_returns.retained_height, None);
        }

        #[test]
        fn block_hash((id, server) in arb_server(), v in proptest::collection::vec(any::<u8>(), 1..32)) {
            // Query using hash
            let data = BTreeMap::from([(0, SingleBlockQuery::Hash(v.clone()))]);
            let data = minicbor::to_vec(data).unwrap();
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("blockchain.block".to_string())
                .data(data)
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let block_returns: BlockReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(block_returns.block.id, BlockIdentifier::new(v, 1));
        }

        #[test]
        fn block_height((id, server) in arb_server(), h in any::<u64>()) {
            // Query using height
            let data = BTreeMap::from([(0, SingleBlockQuery::Height(h))]);
            let data = minicbor::to_vec(data).unwrap();
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("blockchain.block".to_string())
                .data(data)
                .build()
                .unwrap();
            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let block_returns: BlockReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(block_returns.block.id, BlockIdentifier::new(vec![1u8; 8], h));
        }

        #[test]
        fn transaction((id, server) in arb_server(), v in proptest::collection::vec(any::<u8>(), 1..32)) {
            let data = BTreeMap::from([(0, SingleTransactionQuery::Hash(v.clone()))]);
            let data = minicbor::to_vec(data).unwrap();
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("blockchain.transaction".to_string())
                .data(data)
                .build()
                .unwrap();
            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let transaction_returns: TransactionReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(transaction_returns.txn.id.hash, v);
            assert_eq!(transaction_returns.txn.content, None);
        }
    }
}
