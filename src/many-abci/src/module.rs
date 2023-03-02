use clap::__macro_refs::once_cell;
use coset::CborSerializable;
use itertools::Itertools;
use many_client::client::blocking::block_on;
use many_error::ManyError;
use many_identity::{Address, AnonymousIdentity};
use many_modules::r#async::{StatusArgs, StatusReturn};
use many_modules::{abci_frontend, blockchain, r#async};
use many_protocol::{encode_cose_sign1_from_response, ResponseMessage};
use many_types::blockchain::{
    Block, BlockIdentifier, SingleBlockQuery, SingleTransactionQuery, Transaction,
    TransactionIdentifier,
};
use many_types::{blockchain::RangeBlockQuery, SortOrder, Timestamp};
use once_cell::sync::Lazy;
use std::ops::{Bound, RangeBounds};
use tendermint::Time;
use tendermint_rpc::endpoint::tx;
use tendermint_rpc::error::ErrorDetail;
use tendermint_rpc::{query::Query, Client, Error};

const MAXIMUM_BLOCK_COUNT: u64 = 100;
static DEFAULT_BLOCK_LIST_QUERY: Lazy<Query> = Lazy::new(|| Query::gte("block.height", 0));

fn _many_block_from_tendermint_block(block: tendermint::Block) -> Block {
    let height = block.header.height.value();
    let txs_count = block.data.len() as u64;
    let txs = block
        .data
        .into_iter()
        .map(|b| {
            use sha2::Digest;
            let mut hasher = sha2::Sha256::new();
            hasher.update(b);
            Transaction {
                id: TransactionIdentifier {
                    hash: hasher.finalize().to_vec(),
                },
                request: None,
                response: None,
            }
        })
        .collect();
    Block {
        id: BlockIdentifier {
            hash: block.header.hash().into(),
            height,
        },
        parent: if height <= 1 {
            BlockIdentifier::genesis()
        } else {
            BlockIdentifier::new(block.header.last_block_id.unwrap().hash.into(), height - 1)
        },
        app_hash: Some(block.header.app_hash.value()),
        timestamp: Timestamp::new(
            block
                .header
                .time
                .duration_since(Time::unix_epoch())
                .unwrap()
                .as_secs(),
        )
        .unwrap(),
        txs_count,
        txs,
    }
}

fn _tm_order_from_many_order(order: SortOrder) -> tendermint_rpc::Order {
    match order {
        SortOrder::Ascending => tendermint_rpc::Order::Ascending,
        SortOrder::Descending => tendermint_rpc::Order::Descending,
        _ => tendermint_rpc::Order::Ascending,
    }
}

fn _tm_query_from_many_filter(
    filter: RangeBlockQuery,
) -> Result<tendermint_rpc::query::Query, ManyError> {
    let mut query = tendermint_rpc::query::Query::default();
    query = match filter {
        RangeBlockQuery::Height(range) => {
            query = match range.start_bound() {
                Bound::Included(x) => query.and_gte("block.height", *x),
                Bound::Excluded(x) => query.and_gt("block.height", *x),
                _ => query,
            };
            query = match range.end_bound() {
                Bound::Included(x) => query.and_lte("block.height", *x),
                Bound::Excluded(x) => query.and_lt("block.height", *x),
                _ => query,
            };
            query
        }
        RangeBlockQuery::Time(_range) => return Err(ManyError::unknown("Unimplemented")),
    };

    // The default query returns an error (TM 0.35)
    // Return all blocks
    // TODO: Test on TM 0.34 and report an issue in TM-rs if reproducible
    if query.to_string().is_empty() {
        query = DEFAULT_BLOCK_LIST_QUERY.clone();
    }

    Ok(query)
}

pub struct AbciBlockchainModuleImpl<C: Client> {
    client: C,
}

impl<C: Client> AbciBlockchainModuleImpl<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C: Client + Sync> AbciBlockchainModuleImpl<C> {
    /// Call the Tendermint backend to get a transaction based on its query.
    /// Returns a result of Option. Option will be None if the transaction was
    /// not found.
    async fn tx(
        &self,
        query: SingleTransactionQuery,
    ) -> Result<Option<(Vec<u8>, Vec<u8>)>, ManyError> {
        match query {
            SingleTransactionQuery::Hash(hash) => {
                if let Ok(hash) = TryInto::<[u8; 32]>::try_into(hash.clone()) {
                    let tx::Response {
                        tx: tx_request,
                        tx_result,
                        ..
                    } = match self.client.tx(tendermint_rpc::abci::transaction::Hash::new(hash), true).await {
                        Ok(response) => response,
                        // Cannot get more details than response error when the hash is not found.
                        Err(Error(ErrorDetail::Response(_), _tracer)) => return Ok(None),
                        Err(e @ Error(_, _)) => {
                            tracing::error!("abci transport: {e}");
                            return Err(abci_frontend::abci_transport_error(e));
                        }
                    };

                    // Base64 decode is required because of an issue in `tendermint-rs` 0.28.0
                    // TODO: Remove when https://github.com/informalsystems/tendermint-rs/issues/1251 is fixed
                    // let result_tx = general_purpose::STANDARD
                    //    .decode(&tx_result.data)
                    //    .map_err(abci_frontend::abci_transport_error)?;

                    Ok(Some((tx_request.into(), tx_result.data.value().to_vec())))
                } else {
                    Err(ManyError::unknown(format!(
                        "Invalid transaction hash x'{}'.",
                        hex::encode(hash)
                    )))
                }
            }
        }
    }
}

impl<C: Client> Drop for AbciBlockchainModuleImpl<C> {
    fn drop(&mut self) {
        tracing::info!("ABCI Blockchain Module being dropped.");
    }
}

impl<C: Client + Send + Sync> r#async::AsyncModuleBackend for AbciBlockchainModuleImpl<C> {
    fn status(&self, _sender: &Address, args: StatusArgs) -> Result<StatusReturn, ManyError> {
        let hash: Vec<u8> = args.token.into();

        block_on(async {
            let tx_result_data = match self.tx(SingleTransactionQuery::Hash(hash)).await? {
                Some((_, result)) => result,
                None => return Ok(StatusReturn::Unknown),
            };

            tracing::warn!("result: {}", hex::encode(&tx_result_data));
            Ok(StatusReturn::Done {
                response: Box::new(
                    encode_cose_sign1_from_response(
                        ResponseMessage::from_bytes(&tx_result_data)
                            .map_err(abci_frontend::abci_transport_error)?,
                        &AnonymousIdentity,
                    )
                    .map_err(abci_frontend::abci_transport_error)?,
                ),
            })
        })
    }
}

impl<C: Client + Send + Sync> blockchain::BlockchainModuleBackend for AbciBlockchainModuleImpl<C> {
    fn info(&self) -> Result<blockchain::InfoReturns, ManyError> {
        let status = block_on(async { self.client.status().await }).map_err(|e| {
            tracing::error!("abci transport: {}", e.to_string());
            abci_frontend::abci_transport_error(e.to_string())
        })?;

        Ok(blockchain::InfoReturns {
            latest_block: BlockIdentifier {
                hash: status.sync_info.latest_block_hash.as_bytes().to_vec(),
                height: status.sync_info.latest_block_height.value(),
            },
            app_hash: Some(status.sync_info.latest_app_hash.value().to_vec()),
            retained_height: None,
        })
    }

    fn transaction(
        &self,
        args: blockchain::TransactionArgs,
    ) -> Result<blockchain::TransactionReturns, ManyError> {
        let block = block_on(async {
            match args.query {
                SingleTransactionQuery::Hash(hash) => {
                    if let Ok(hash) = TryInto::<[u8; 32]>::try_into(hash) {
                        self.client
                            .tx(tendermint_rpc::abci::transaction::Hash::new(hash), true)
                            .await
                            .map_err(|e| {
                                tracing::error!("abci transport: {}", e.to_string());
                                abci_frontend::abci_transport_error(e.to_string())
                            })
                    } else {
                        Err(ManyError::unknown("Invalid transaction hash .".to_string()))
                    }
                }
            }
        })?;

        let tx_hash = block.hash.as_bytes().to_vec();
        Ok(blockchain::TransactionReturns {
            txn: Transaction {
                id: TransactionIdentifier { hash: tx_hash },
                request: None,
                response: None,
            },
        })
    }

    fn block(&self, args: blockchain::BlockArgs) -> Result<blockchain::BlockReturns, ManyError> {
        let block = block_on(async {
            match args.query {
                SingleBlockQuery::Hash(hash) => {
                    if let Ok(hash) = TryInto::<[u8; 32]>::try_into(hash) {
                        self.client
                            .block_by_hash(tendermint::Hash::Sha256(hash))
                            .await
                            .map_err(|e| {
                                tracing::error!("abci transport: {}", e.to_string());
                                abci_frontend::abci_transport_error(e.to_string())
                            })
                            .map(|search| search.block)
                    } else {
                        Err(ManyError::unknown("Invalid hash length.".to_string()))
                    }
                }
                SingleBlockQuery::Height(height) => self
                    .client
                    .block(height as u32)
                    .await
                    .map_err(|e| {
                        tracing::error!("abci transport: {}", e.to_string());
                        abci_frontend::abci_transport_error(e.to_string())
                    })
                    .map(|x| Some(x.block)),
            }
        })?;

        if let Some(block) = block {
            let block = _many_block_from_tendermint_block(block);
            Ok(blockchain::BlockReturns { block })
        } else {
            Err(blockchain::unknown_block())
        }
    }

    fn list(&self, args: blockchain::ListArgs) -> Result<blockchain::ListReturns, ManyError> {
        let blockchain::ListArgs {
            count,
            order,
            filter,
        } = args;

        let count = count.map_or(MAXIMUM_BLOCK_COUNT, |c| {
            std::cmp::min(c, MAXIMUM_BLOCK_COUNT)
        });

        // We can get maximum u8::MAX blocks per page and a maximum of u32::MAX pages
        // Find the correct number of pages and count
        let (pages, count): (u32, u8) = if count > u8::MAX as u64 {
            (
                num_integer::div_ceil(count, u8::MAX as u64)
                    .try_into()
                    .map_err(|_| ManyError::unknown("Unable to cast u64 to u32"))?,
                u8::MAX,
            )
        } else {
            (1u32, count as u8)
        };

        let order = order.map_or(tendermint_rpc::Order::Ascending, _tm_order_from_many_order);

        let query = filter.map_or(
            Ok(DEFAULT_BLOCK_LIST_QUERY.clone()),
            _tm_query_from_many_filter,
        )?;

        let (status, block) = block_on(async move {
            let status = self.client.status().await;
            let block = self.client.block_search(query, pages, count, order).await;
            (status, block)
        });

        let blocks = block
            .map_err(ManyError::unknown)?
            .blocks
            .into_iter()
            .map(|x| _many_block_from_tendermint_block(x.block))
            .collect_vec();

        Ok(blockchain::ListReturns {
            height: status
                .map_err(ManyError::unknown)?
                .sync_info
                .latest_block_height
                .value(),
            blocks,
        })
    }

    fn request(
        &self,
        args: blockchain::RequestArgs,
    ) -> Result<blockchain::RequestReturns, ManyError> {
        let (request, _) = block_on(async { self.tx(args.query).await })?
            .ok_or_else(|| ManyError::unknown("Hash not found."))?;
        tracing::debug!("blockchain.request: {}", hex::encode(&request));
        Ok(blockchain::RequestReturns { request })
    }

    fn response(
        &self,
        args: blockchain::ResponseArgs,
    ) -> Result<blockchain::ResponseReturns, ManyError> {
        let (_, response) = block_on(async { self.tx(args.query).await })?
            .ok_or_else(|| ManyError::unknown("Hash not found."))?;

        tracing::debug!("blockchain.response: {}", hex::encode(&response));

        let response: ResponseMessage =
            minicbor::decode(&response).map_err(ManyError::deserialization_error)?;
        Ok(blockchain::ResponseReturns {
            response: encode_cose_sign1_from_response(response, &AnonymousIdentity)?
                .to_vec()
                .map_err(ManyError::serialization_error)?,
        })
    }
}
