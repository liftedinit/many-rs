use crate::error;
use crate::storage::{url_for_website, WebStorage, HTTP_ROOT};
use base64::{engine::general_purpose, Engine as _};
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, BeginBlockReturn, EndpointInfo, InitChainReturn,
    ManyAbciModuleBackend,
};
use many_modules::kvstore::{GetArgs, GetReturns, KvStoreModuleBackend, QueryArgs, QueryReturns};
use many_modules::web::{
    DeployArgs, DeployReturns, InfoArg, InfoReturns, ListArgs, ListReturns, RemoveArgs,
    RemoveReturns, UpdateArgs, UpdateReturns, WebCommandsModuleBackend, WebModuleBackend,
};
use many_types::web::{WebDeploymentInfo, WebDeploymentSource};
use many_types::Timestamp;
use sha2::Digest;
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;
use tempfile::Builder;
use tracing::{info, trace};
use trust_dns_resolver::Name;

const MAXIMUM_WEB_COUNT: usize = 100;

pub mod allow_addrs;
pub mod events;

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
            height = storage.get_height()?,
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
                // KvStore
                ("kvstore.get".to_string(), EndpointInfo { is_command: false }),
                ("kvstore.info".to_string(), EndpointInfo { is_command: false }),
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
            self.storage.get_height()?
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
            storage.get_height()?,
            hex::encode(storage.hash()).as_str()
        );
        Ok(AbciInfo {
            height: storage.get_height()?,
            hash: storage.hash().into(),
        })
    }

    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError> {
        let result = self.storage.commit()?;

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

fn is_alphanumeric_or_symbols(s: &str) -> Result<(), ManyError> {
    trace!("Checking {s} is alphanumeric or symbols");
    if !all_alphanumeric_or_symbols(s) {
        return Err(error::not_alphanumeric_or_symbols(s));
    }
    Ok(())
}

fn extract_valid_domain(domain: Option<String>) -> Result<Option<String>, ManyError> {
    if let Some(domain) = domain {
        Ok(Some(
            Name::from_utf8(domain)
                .map_err(error::invalid_domain)?
                .to_string(),
        ))
    } else {
        Ok(None)
    }
}

fn _transform_site_name(site_name: String) -> String {
    site_name.to_lowercase().trim().replace(' ', "_")
}

fn _prepare_deployment(
    site_name: String,
    site_description: Option<String>,
    source: WebDeploymentSource,
    serve_path: impl AsRef<Path>,
) -> Result<String, ManyError> {
    is_alphanumeric_or_symbols(&site_name)?;
    if let Some(site_description) = &site_description {
        is_alphanumeric_or_symbols(site_description)?;
    }

    trace!("Checking site source");
    let source_hash = match &source {
        WebDeploymentSource::Archive(bytes) => {
            zip::ZipArchive::new(Cursor::new(bytes.as_slice()))
                .map_err(error::invalid_zip_file)?
                .extract(&serve_path)
                .map_err(error::unable_to_extract_zip_file)?;
            hex::encode(sha2::Sha256::digest(bytes.as_slice()).as_slice())
        }
    };

    // Look for `index.html` in the root of the serve path
    let index_path = serve_path.as_ref().join("index.html");
    if !index_path.exists() {
        return Err(error::missing_index_html());
    }

    Ok(source_hash)
}

impl WebModuleBackend for WebModuleImpl {
    fn info(&self, _sender: &Address, _args: InfoArg) -> Result<InfoReturns, ManyError> {
        Ok(InfoReturns {
            hash: self.storage.hash().into(),
        })
    }

    fn list(&self, _sender: &Address, args: ListArgs) -> Result<ListReturns, ManyError> {
        Ok(ListReturns {
            total_count: self.storage.get_deployment_count()?,
            deployments: self
                .storage
                .list(args.order.unwrap_or_default(), args.filter)
                .map(|(_, meta)| meta)
                .take(args.count.unwrap_or(MAXIMUM_WEB_COUNT))
                .collect(),
        })
    }
}

impl WebCommandsModuleBackend for WebModuleImpl {
    fn deploy(&mut self, sender: &Address, args: DeployArgs) -> Result<DeployReturns, ManyError> {
        let DeployArgs {
            owner,
            site_name,
            site_description,
            source,
            memo,
            domain,
        } = args;

        // Check that the sender is the owner, for now.
        // TODO: Support accounts
        let owner = if let Some(owner) = owner {
            if sender != &owner {
                return Err(error::invalid_owner(owner));
            }
            sender
        } else {
            sender
        };

        let domain = extract_valid_domain(domain)?;

        if site_name.len() > 12 {
            return Err(error::site_name_too_long(site_name));
        }

        let site_name = _transform_site_name(site_name);

        if self.storage.site_exists(owner, &site_name)? {
            return Err(error::existent_site(site_name));
        }

        let tmpdir = Builder::new()
            .prefix("dweb-")
            .tempdir()
            .map_err(error::unable_to_create_tempdir)?;
        trace!(
            "Created temporary directory {path}",
            path = tmpdir.path().display()
        );

        let serve_path = tmpdir.path().to_path_buf();

        let source_hash = _prepare_deployment(
            site_name.clone(),
            site_description.clone(),
            source,
            &serve_path,
        )?;
        self.storage.store_website(
            sender,
            site_name.clone(),
            site_description.clone(),
            memo,
            source_hash,
            serve_path,
            domain.clone(),
        )?;

        let url = url_for_website(sender, &site_name);

        Ok(DeployReturns {
            info: WebDeploymentInfo {
                owner: *owner,
                site_name,
                site_description,
                url: Some(url),
                domain,
            },
        })
    }

    fn remove(&mut self, sender: &Address, args: RemoveArgs) -> Result<RemoveReturns, ManyError> {
        let RemoveArgs {
            owner,
            site_name,
            memo,
        } = args;

        // Check that the sender is the owner, for now.
        // TODO: Support accounts
        if let Some(owner) = owner {
            if sender != &owner {
                return Err(error::invalid_owner(owner));
            }
        }

        let site_name = _transform_site_name(site_name);
        self.storage.remove_website(sender, site_name, memo)?;
        Ok(RemoveReturns {})
    }

    fn update(&mut self, sender: &Address, args: UpdateArgs) -> Result<UpdateReturns, ManyError> {
        let UpdateArgs {
            owner,
            site_name,
            site_description,
            source,
            memo,
            domain,
        } = args;

        // Check that the sender is the owner, for now.
        // TODO: Support accounts
        let owner = if let Some(owner) = owner {
            if sender != &owner {
                return Err(error::invalid_owner(owner));
            }
            sender
        } else {
            sender
        };

        let domain = extract_valid_domain(domain)?;

        if site_name.len() > 12 {
            return Err(error::site_name_too_long(site_name));
        }

        let site_name = _transform_site_name(site_name);

        // Don't update an nonexistent site.
        if !self.storage.site_exists(owner, &site_name)? {
            return Err(error::nonexistent_site(site_name));
        }

        let tmpdir = Builder::new()
            .prefix("dweb-")
            .tempdir()
            .map_err(error::unable_to_create_tempdir)?;
        trace!(
            "Created temporary directory {path}",
            path = tmpdir.path().display()
        );

        let serve_path = tmpdir.path().to_path_buf();

        let source_hash = _prepare_deployment(
            site_name.clone(),
            site_description.clone(),
            source,
            &serve_path,
        )?;
        self.storage.update_website(
            owner,
            site_name.clone(),
            site_description.clone(),
            memo,
            source_hash,
            serve_path,
            domain.clone(),
        )?;

        let url = url_for_website(sender, &site_name);
        Ok(UpdateReturns {
            info: WebDeploymentInfo {
                owner: *owner,
                site_name,
                site_description,
                url: Some(url),
                domain,
            },
        })
    }
}

impl KvStoreModuleBackend for WebModuleImpl {
    fn info(
        &self,
        _sender: &Address,
        _args: many_modules::kvstore::InfoArg,
    ) -> Result<many_modules::kvstore::InfoReturns, ManyError> {
        Ok(many_modules::kvstore::InfoReturns {
            hash: self.storage.hash().into(),
        })
    }

    fn get(&self, _sender: &Address, args: GetArgs) -> Result<GetReturns, ManyError> {
        let GetArgs { key } = args;

        if !key.starts_with(HTTP_ROOT.as_ref()) {
            return Err(error::key_should_start_with_http());
        }

        let value = self.storage.get(key.as_slice())?;
        match value {
            Some(value) => Ok(GetReturns {
                value: Some(
                    general_purpose::STANDARD
                        .decode(value)
                        .map_err(ManyError::deserialization_error)?
                        .into(),
                ),
            }),
            None => Ok(GetReturns { value: None }),
        }
    }

    // We do not expose this endpoint
    fn query(&self, _sender: &Address, _args: QueryArgs) -> Result<QueryReturns, ManyError> {
        Err(ManyError::unknown("Unimplemented"))
    }

    // We do not expose this endpoint
    fn list(
        &self,
        _sender: &Address,
        _args: many_modules::kvstore::list::ListArgs,
    ) -> Result<many_modules::kvstore::list::ListReturns, ManyError> {
        Err(ManyError::unknown("Unimplemented"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn valid_domain() {
        let domain = "foobar.com";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(domain.to_string()));
    }

    #[test]
    fn valid_subdomain() {
        let domain = "foo.bar.com";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(domain.to_string()));
    }

    #[test]
    fn valid_long_domain() {
        let domain = "a".repeat(63)
            + "."
            + &*"b".repeat(63)
            + "."
            + &*"c".repeat(63)
            + "."
            + &*"d".repeat(62);
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(domain.to_string()));
    }

    #[test]
    fn invalid_domain_with_underscore() {
        let domain = "foo_bar.com";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn invalid_domain_with_underscore_and_hyphen() {
        let domain = "foo_bar-bar.com";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn invalid_domain_with_underscore_and_hyphen_and_dot() {
        let domain = "foo_bar-bar.com.";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn invalid_domain_with_underscore_and_hyphen_and_dot_and_space() {
        let domain = "foo_bar-bar.com. ";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn invalid_domain_with_underscore_and_hyphen_and_dot_and_space_and_newline() {
        let domain = "foo_bar-bar.com. \n";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn invalid_domain_with_underscore_and_hyphen_and_dot_and_space_and_newline_and_tab() {
        let domain = "foo_bar-bar.com. \n\t";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn invalid_domain_with_underscore_and_hyphen_and_dot_and_space_and_newline_and_tab_and_carriage_return(
    ) {
        let domain = "foo_bar-bar.com. \n\t\r";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn invalid_domain_label_too_long() {
        let domain = "a".repeat(64) + ".com";
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn invalid_domain_too_long() {
        let domain = "a".repeat(63)
            + "."
            + &*"b".repeat(63)
            + "."
            + &*"c".repeat(63)
            + "."
            + &*"d".repeat(63);
        let result = super::extract_valid_domain(Some(domain.to_string()));
        assert!(result.is_err());
    }
}
