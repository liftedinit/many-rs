// use git2::Repository;
use crate::error;
use crate::storage::{WebStorage, HTTP_ROOT};
use git2::Repository;
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, BeginBlockReturn, EndpointInfo, InitChainReturn,
    ManyAbciModuleBackend,
};
use many_modules::kvstore::{GetArgs, GetReturns, KvStoreModuleBackend, QueryArgs, QueryReturns};
use many_modules::web::{
    DeployArgs, DeployReturns, InfoArg, InfoReturns, ListArgs, ListReturns, RemoveArgs,
    RemoveReturns, WebModuleBackend,
};
use many_types::web::WebDeploymentSource;
use many_types::Timestamp;
use std::collections::BTreeMap;
use std::path::Path;
use tracing::info;

pub mod allow_addrs;

// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    identity: Address,
    hash: Option<String>,
}

#[derive(Debug)]
pub struct WebModuleImpl {
    storage: WebStorage,
}

impl WebModuleImpl {
    pub fn load<P: AsRef<Path>>(
        persistent_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage =
            WebStorage::load(persistent_store_path, blockchain).map_err(ManyError::unknown)?;

        Ok(Self { storage })
    }

    pub fn new<P: AsRef<Path>>(
        initial_state: InitialStateJson,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage = WebStorage::new(initial_state.identity, persistence_store_path, blockchain)
            .map_err(ManyError::unknown)?;

        if let Some(h) = initial_state.hash {
            // Verify the hash.
            let actual = hex::encode(storage.hash());
            if actual != h {
                return Err(error::invalid_initial_hash(h, actual));
            }
        }

        info!(
            height = storage.get_height(),
            hash = hex::encode(storage.hash()).as_str()
        );

        Ok(Self { storage })
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl ManyAbciModuleBackend for WebModuleImpl {
    #[rustfmt::skip]
    fn init(&mut self) -> Result<AbciInit, ManyError> {
        Ok(AbciInit {
            endpoints: BTreeMap::from([
                ("web.info".to_string(), EndpointInfo { is_command: false }),
                ("web.deploy".to_string(), EndpointInfo { is_command: true }),
                ("web.remove".to_string(), EndpointInfo { is_command: true }),
                ("web.list".to_string(), EndpointInfo { is_command: false }),
                //
                // Events
                ("events.info".to_string(), EndpointInfo { is_command: false }),
                ("events.list".to_string(), EndpointInfo { is_command: false }),
            ]),
        })
    }

    fn init_chain(&mut self) -> Result<InitChainReturn, ManyError> {
        info!("abci.init_chain()",);
        Ok(InitChainReturn {})
    }

    fn begin_block(&mut self, info: AbciBlock) -> Result<BeginBlockReturn, ManyError> {
        let time = info.time;
        info!(
            "abci.block_begin(): time={:?} curr_height={}",
            time,
            self.storage.get_height()
        );

        if let Some(time) = time {
            let time = Timestamp::new(time)?;
            self.storage.set_time(time);
        }

        Ok(BeginBlockReturn {})
    }

    fn info(&self) -> Result<AbciInfo, ManyError> {
        let storage = &self.storage;

        info!(
            "abci.info(): height={} hash={}",
            storage.get_height(),
            hex::encode(storage.hash()).as_str()
        );
        Ok(AbciInfo {
            height: storage.get_height(),
            hash: storage.hash().into(),
        })
    }

    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError> {
        let result = self.storage.commit();

        info!(
            "abci.commit(): retain_height={} hash={}",
            result.retain_height,
            hex::encode(result.hash.as_slice()).as_str()
        );
        Ok(result)
    }
}

fn all_alphanumeric_or_symbols(input: &str) -> bool {
    input
        .chars()
        .all(|c| c.is_alphanumeric() || c.is_ascii_punctuation() || c.is_ascii_whitespace())
}

impl WebModuleBackend for WebModuleImpl {
    fn info(&self, sender: &Address, args: InfoArg) -> Result<InfoReturns, ManyError> {
        Ok(InfoReturns {
            hash: self.storage.hash().into(),
        })
    }

    fn deploy(&mut self, sender: &Address, args: DeployArgs) -> Result<DeployReturns, ManyError> {
        let DeployArgs {
            site_name,
            site_description,
            source,
        } = args;

        if !all_alphanumeric_or_symbols(&site_name) {
            return Err(error::invalid_site_name(site_name));
        }

        if let Some(site_description) = &site_description {
            if !all_alphanumeric_or_symbols(&site_description) {
                return Err(error::invalid_site_description(site_description));
            }
        }

        match source {
            WebDeploymentSource::GitHub(source) => {
                let tmpdir = tempfile::tempdir().map_err(error::unable_to_create_tempdir)?;
                let repo = Repository::clone(&source, tmpdir.path())
                    .map_err(error::unable_to_clone_repository)?;
                self.storage
                    .store_website(sender, &site_name, &site_description, repo.path())?;
            }
        }

        Ok(DeployReturns {
            url: "FIXME".to_string(),
        })
    }

    fn remove(&mut self, sender: &Address, args: RemoveArgs) -> Result<RemoveReturns, ManyError> {
        let RemoveArgs { site_name } = args;
        self.storage.remove_website(sender, &site_name)?;
        Ok(RemoveReturns {})
    }

    fn list(&self, sender: &Address, args: ListArgs) -> Result<ListReturns, ManyError> {
        todo!()
    }
}

impl KvStoreModuleBackend for WebModuleImpl {
    fn info(
        &self,
        sender: &Address,
        args: many_modules::kvstore::InfoArg,
    ) -> Result<many_modules::kvstore::InfoReturns, ManyError> {
        Ok(many_modules::kvstore::InfoReturns {
            hash: self.storage.hash().into(),
        })
    }

    fn get(&self, sender: &Address, args: GetArgs) -> Result<GetReturns, ManyError> {
        let GetArgs { key } = args;

        if !key.starts_with(HTTP_ROOT.as_ref()) {
            return Err(error::key_should_start_with_http());
        }

        Ok(GetReturns {
            value: self.storage.get(key.as_slice())?.map(|v| v.into()),
        })
    }

    fn query(&self, sender: &Address, args: QueryArgs) -> Result<QueryReturns, ManyError> {
        Err(ManyError::unknown("Unimplemented")) // TODO
    }

    fn list(
        &self,
        sender: &Address,
        args: many_modules::kvstore::list::ListArgs,
    ) -> Result<many_modules::kvstore::list::ListReturns, ManyError> {
        Err(ManyError::unknown("Unimplemented")) // TODO
    }
}
