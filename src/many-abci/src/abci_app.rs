use crate::migration::error_code::LEGACY_ERROR_CODE_TRIGGER;
use crate::migration::{AbciAppMigrations, MIGRATIONS};
use coset::{CborSerializable, CoseSign1};
use many_client::client::blocking::{block_on, ManyClient};
use many_error::{ManyError, ManyErrorCode};
use many_identity::{Address, AnonymousIdentity};
use many_migration::MigrationConfig;
use many_modules::abci_backend::{AbciBlock, AbciCommitInfo, AbciInfo};
use many_protocol::ResponseMessage;
use reqwest::{IntoUrl, Url};
use std::sync::{Arc, RwLock};
use tendermint_abci::Application;
use tendermint_proto::abci::*;
use tracing::{debug, error};

lazy_static::lazy_static!(
    static ref EPOCH: many_types::Timestamp = many_types::Timestamp::new(0).unwrap();
);

fn get_abci_info_(client: &ManyClient<AnonymousIdentity>) -> Result<AbciInfo, ManyError> {
    client
        .call_("abci.info", ())
        .and_then(|payload| minicbor::decode(&payload).map_err(ManyError::deserialization_error))
}

#[derive(Debug, Clone)]
pub struct AbciApp {
    app_name: String,
    many_client: ManyClient<AnonymousIdentity>,
    many_url: Url,

    /// We need interior mutability, safely.
    migrations: Arc<RwLock<AbciAppMigrations>>,
}

impl AbciApp {
    /// Constructor.
    pub fn create<U>(
        many_url: U,
        server_id: Address,
        migration_config: Option<MigrationConfig>,
    ) -> Result<Self, String>
    where
        U: IntoUrl,
    {
        let many_url = many_url.into_url().map_err(|e| e.to_string())?;

        // TODO: Get the server ID from the many server.
        // let server_id = if server_id.is_anonymous() {
        //     server_id
        // } else {
        //     server_id
        // };

        let many_client = ManyClient::new(many_url.clone(), server_id, AnonymousIdentity)?;
        let status = many_client.status().map_err(|x| x.to_string())?;
        let app_name = status.name;

        let migrations = RwLock::new({
            let AbciInfo { height, .. } = get_abci_info_(&many_client)
                .map_err(|e| format!("Unable to call abci.info: {e}"))?;

            let migrations = migration_config
                .map_or_else(AbciAppMigrations::empty, |config| {
                    AbciAppMigrations::load(&MIGRATIONS, config, height)
                })
                .map_err(|e| format!("Unable to load migrations: {e}"))?;
            debug!("Final migrations: {:?}", migrations);
            migrations
        });

        Ok(Self {
            app_name,
            many_url,
            many_client,
            migrations: Arc::new(migrations),
        })
    }
}

impl Application for AbciApp {
    fn info(&self, request: RequestInfo) -> ResponseInfo {
        debug!(
            "Got info request. Tendermint version: {}; Block version: {}; P2P version: {}",
            request.version, request.block_version, request.p2p_version
        );

        let AbciInfo { height, hash } = match get_abci_info_(&self.many_client) {
            Ok(x) => x,
            Err(err) => {
                return ResponseInfo {
                    data: format!("An error occurred during call to abci.info:\n{err}"),
                    ..Default::default()
                }
            }
        };

        ResponseInfo {
            data: format!("many-abci-bridge({})", self.app_name),
            version: env!("CARGO_PKG_VERSION").to_string(),
            app_version: 1,
            last_block_height: height as i64,
            last_block_app_hash: hash.to_vec().into(),
        }
    }
    fn init_chain(&self, _request: RequestInitChain) -> ResponseInitChain {
        Default::default()
    }
    fn query(&self, request: RequestQuery) -> ResponseQuery {
        let cose = match CoseSign1::from_slice(&request.data) {
            Ok(x) => x,
            Err(err) => {
                return ResponseQuery {
                    code: 2,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        let value = match block_on(many_client::client::send_envelope(
            self.many_url.clone(),
            cose,
        )) {
            Ok(cose_sign) => cose_sign,

            Err(err) => {
                return ResponseQuery {
                    code: 3,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };

        match value.to_vec() {
            Ok(value) => ResponseQuery {
                code: 0,
                value: value.into(),
                ..Default::default()
            },
            Err(err) => ResponseQuery {
                code: 1,
                log: err.to_string(),
                ..Default::default()
            },
        }
    }

    fn begin_block(&self, request: RequestBeginBlock) -> ResponseBeginBlock {
        let (time, height) = request
            .header
            .map(|x| {
                let time = x.time.map(|x| x.seconds as u64);
                let height = Some(if x.height > 0 { x.height as u64 } else { 0 });

                (time, height)
            })
            .unwrap_or((None, None));

        if let Some(height) = height {
            if let Ok(mut m) = self.migrations.write() {
                // Since it's impossible to truly handle error here, and
                // we don't actually want to panic, just ignore any errors.
                let _ = m.update_at_height(&mut (), height);
            } else {
                error!("Migration: Could not acquire migration lock...");
            }
        }

        let block = AbciBlock { time };
        let _ = self.many_client.call_("abci.beginBlock", block);
        ResponseBeginBlock { events: vec![] }
    }

    fn deliver_tx(&self, request: RequestDeliverTx) -> ResponseDeliverTx {
        let cose = match CoseSign1::from_slice(&request.tx) {
            Ok(x) => x,
            Err(err) => {
                return ResponseDeliverTx {
                    code: 2,
                    log: err.to_string(),
                    ..Default::default()
                }
            }
        };
        match block_on(many_client::client::send_envelope(
            self.many_url.clone(),
            cose,
        )) {
            Ok(cose_sign) => {
                let payload = cose_sign.payload.unwrap_or_default();
                let mut response = ResponseMessage::from_bytes(&payload).unwrap_or_default();

                // Consensus will sign the result, so the `from` field is unnecessary.
                response.from = Address::anonymous();
                // The version is ignored and removed.
                response.version = None;
                // The timestamp MIGHT differ between two nodes so we just force it to be 0.
                response.timestamp = Some(*EPOCH);

                // Check whether we need to apply a correction to the error code decoding
                // logic.
                // A bug in the Error module was fixed in
                //     https://github.com/liftedinit/many-rs/pull/177
                // which meant we started decoding errors properly, but in production
                // the ledger was genesis before.
                if let Ok(m) = self.migrations.read() {
                    if m.is_active(&LEGACY_ERROR_CODE_TRIGGER) {
                        response.data = match response.data {
                            Err(err) => {
                                if err.code().is_attribute_specific() {
                                    Err(err.with_code(ManyErrorCode::Unknown))
                                } else {
                                    Err(err)
                                }
                            }
                            x => x,
                        };
                    }
                }

                if let Ok(data) = response.to_bytes() {
                    ResponseDeliverTx {
                        code: 0,
                        data: data.into(),
                        ..Default::default()
                    }
                } else {
                    ResponseDeliverTx {
                        code: 3,
                        ..Default::default()
                    }
                }
            }
            Err(err) => ResponseDeliverTx {
                code: 1,
                data: vec![].into(),
                log: err.to_string(),
                ..Default::default()
            },
        }
    }

    fn end_block(&self, _request: RequestEndBlock) -> ResponseEndBlock {
        let _ = self.many_client.call_("abci.endBlock", ());
        Default::default()
    }

    fn flush(&self) -> ResponseFlush {
        Default::default()
    }

    fn commit(&self) -> ResponseCommit {
        self.many_client.call_("abci.commit", ()).map_or_else(
            |err| ResponseCommit {
                data: err.to_string().into_bytes().into(),
                retain_height: 0,
            },
            |msg| {
                let info: AbciCommitInfo = minicbor::decode(&msg).unwrap();
                ResponseCommit {
                    data: info.hash.to_vec().into(),
                    retain_height: info.retain_height as i64,
                }
            },
        )
    }
}
