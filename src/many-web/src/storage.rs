use crate::error;
use crate::storage::iterator::WebIterator;
use base64::engine::general_purpose;
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::AbciCommitInfo;
use many_modules::events::EventId;
use many_types::web::{WebDeploymentFilter, WebDeploymentInfo};
use many_types::{SortOrder, Timestamp};
use merk::{BatchEntry, Op};
use std::fs;
use std::io::Write;
use std::path::Path;
use tracing::trace;
use walkdir::{DirEntry, WalkDir};

pub mod iterator;

pub const HTTP_ROOT: &str = "/http"; // Where website files are stored.
const META_ROOT: &str = "/meta"; // Where website metadata are stored.

fn key_for_website(owner: &Address, site_name: &str) -> Vec<u8> {
    format!("{HTTP_ROOT}/{owner}/{site_name}/").into_bytes()
}

fn key_for_website_file(owner: &Address, site_name: &str, file_name: &str) -> String {
    format!("{HTTP_ROOT}/{owner}/{site_name}/{file_name}")
}

fn key_for_website_meta(owner: &Address, site_name: &str) -> Vec<u8> {
    format!("{META_ROOT}/{owner}/{site_name}").into_bytes()
}

pub fn url_for_website(owner: &Address, site_name: &str) -> String {
    format!("https://{}.{}.web.liftedinit.tech", site_name, owner)
}

pub struct WebStorage {
    persistent_store: merk::Merk,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,

    latest_event_id: EventId,
    current_time: Option<Timestamp>,
    current_hash: Option<Vec<u8>>,
    #[allow(dead_code)]
    next_subresource: u32,
    #[allow(dead_code)]
    root_identity: Address,
}

impl std::fmt::Debug for WebStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebStorage").finish()
    }
}

impl WebStorage {
    #[inline]
    pub fn set_time(&mut self, time: Timestamp) {
        self.current_time = Some(time);
    }

    pub fn load<P: AsRef<Path>>(persistent_path: P, blockchain: bool) -> Result<Self, String> {
        let persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let next_subresource = persistent_store
            .get(b"/config/subresource_id")
            .unwrap()
            .map_or(0, |x| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(x.as_slice());
                u32::from_be_bytes(bytes)
            });

        let root_identity: Address = Address::from_bytes(
            &persistent_store
                .get(b"/config/identity")
                .expect("Could not open storage.")
                .expect("Could not find key '/config/identity' in storage."),
        )
        .map_err(|e| e.to_string())?;

        let latest_event_id = minicbor::decode(
            &persistent_store
                .get(b"/latest_event_id")
                .expect("Could not open storage.")
                .expect("Could not find key '/latest_event_id'"),
        )
        .map_err(|e| e.to_string())?;

        Ok(Self {
            persistent_store,
            blockchain,
            current_time: None,
            current_hash: None,
            latest_event_id,
            next_subresource,
            root_identity,
        })
    }

    pub fn new<P: AsRef<Path>>(
        identity: Address,
        persistent_path: P,
        blockchain: bool,
    ) -> Result<Self, String> {
        let mut persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let batch: Vec<BatchEntry> =
            vec![(b"/config/identity".to_vec(), Op::Put(identity.to_vec()))];

        persistent_store
            .apply(batch.as_slice())
            .map_err(|e| e.to_string())?;

        let latest_event_id = EventId::from(vec![0]);
        persistent_store
            .apply(&[(
                b"/latest_event_id".to_vec(),
                Op::Put(minicbor::to_vec(&latest_event_id).expect("Unable to encode event id")),
            )])
            .unwrap();

        persistent_store.commit(&[]).map_err(|e| e.to_string())?;

        Ok(Self {
            persistent_store,
            blockchain,
            current_time: None,
            current_hash: None,
            latest_event_id,
            next_subresource: 0,
            root_identity: identity,
        })
    }

    fn inc_height(&mut self) -> u64 {
        let current_height = self.get_height();
        self.persistent_store
            .apply(&[(
                b"/height".to_vec(),
                Op::Put((current_height + 1).to_be_bytes().to_vec()),
            )])
            .unwrap();
        current_height
    }

    pub fn get_height(&self) -> u64 {
        self.persistent_store
            .get(b"/height")
            .unwrap()
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            })
    }

    pub fn commit(&mut self) -> AbciCommitInfo {
        let _ = self.inc_height();
        self.persistent_store
            .apply(&[(
                b"/latest_event_id".to_vec(),
                Op::Put(
                    minicbor::to_vec(&self.latest_event_id).expect("Unable to encode event id"),
                ),
            )])
            .unwrap();
        self.persistent_store.commit(&[]).unwrap();

        let retain_height = 0;
        let hash = self.persistent_store.root_hash().to_vec();
        self.current_hash = Some(hash.clone());

        AbciCommitInfo {
            retain_height,
            hash: hash.into(),
        }
    }

    pub fn hash(&self) -> Vec<u8> {
        self.current_hash
            .as_ref()
            .map_or_else(|| self.persistent_store.root_hash().to_vec(), |x| x.clone())
    }

    // Returns if a directory entry is hidden or not.
    fn is_hidden(entry: &DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    }

    pub fn store_website(
        &mut self,
        owner: &Address,
        site_name: String,
        site_description: Option<String>,
        path: impl AsRef<Path>,
    ) -> Result<(), ManyError> {
        let mut batch: Vec<BatchEntry> = Vec::new();

        // Walk the directory tree, ignoring hidden files and directories.
        // Add each file content to the batch as base64
        trace!("Walking directory tree");
        for entry in WalkDir::new(&path)
            .into_iter()
            .filter_entry(|e| !Self::is_hidden(e))
        {
            let entry = entry.map_err(error::unable_to_read_entry)?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                trace!("Skipping directory");
                continue;
            }

            let file_path = entry_path
                .strip_prefix(&path)
                .map_err(error::unable_to_strip_prefix)?
                .to_str()
                .ok_or_else(error::unable_to_convert_to_str)?;
            trace!("Found file {}", file_path);

            trace!("Encoding file");
            let mut enc = base64::write::EncoderWriter::new(Vec::new(), &general_purpose::STANDARD);
            enc.write_all(&fs::read(entry_path).map_err(error::io_error)?)
                .map_err(error::io_error)?;

            trace!("Finished encoding file");
            let data = enc.finish().map_err(ManyError::unknown)?;

            trace!(
                "Storing file to {}",
                key_for_website_file(owner, &site_name, file_path)
            );
            batch.push((
                key_for_website_file(owner, &site_name, file_path).into_bytes(),
                Op::Put(data),
            ));
        }

        let url = url_for_website(owner, &site_name);

        trace!("Adding website meta to batch");
        batch.push((
            key_for_website_meta(owner, &site_name),
            Op::Put(
                minicbor::to_vec(WebDeploymentInfo {
                    owner: *owner,
                    site_name,
                    site_description,
                    url: Some(url),
                })
                .map_err(ManyError::serialization_error)?,
            ),
        ));

        trace!("Sorting batch");
        batch.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

        trace!("Applying batch");
        self.persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        // TODO: Refactor
        if !self.blockchain {
            trace!("Committing batch");
            self.persistent_store
                .commit(&[])
                .map_err(error::storage_commit_failed)?;
        }

        Ok(())
    }

    pub fn remove_website<S: AsRef<str>>(
        &mut self,
        owner: &Address,
        site_name: S,
    ) -> Result<(), ManyError> {
        trace!("Removing website {}", site_name.as_ref());
        let it = WebIterator::website_files(&self.persistent_store, owner, &site_name);
        let mut batch: Vec<BatchEntry> = Vec::new();

        // Remove each file of the website
        for item in it {
            let (key, _) = item.map_err(error::storage_get_failed)?;
            batch.push((key.to_vec(), Op::Delete));
        }

        trace!("Removing website meta");
        batch.push((key_for_website_meta(owner, site_name.as_ref()), Op::Delete));

        self.persistent_store
            .apply(&batch)
            .map_err(error::storage_apply_failed)?;

        // TODO: Refactor
        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .map_err(error::storage_commit_failed)?;
        }

        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        self.persistent_store
            .get(key)
            .map_err(error::storage_get_failed)
    }

    pub fn list(
        &self,
        order: SortOrder,
        filter: Option<Vec<WebDeploymentFilter>>,
    ) -> impl Iterator<Item = (Vec<u8>, WebDeploymentInfo)> + '_ {
        WebIterator::meta(&self.persistent_store, order).filter_map(move |item| {
            let (k, v) = item.ok()?; // Note: Errors are silently ignored
            let meta: WebDeploymentInfo = minicbor::decode(&v).ok()?; // Note: Errors are silently ignored
            if let Some(filters) = &filter {
                if !filters.is_empty() {
                    return if filters.iter().all(|f| filter_item(f, &k, &meta)) {
                        Some((k.into_vec(), meta))
                    } else {
                        None
                    };
                }
            }
            Some((k.into_vec(), meta))
        })
    }
}

fn filter_item(filter: &WebDeploymentFilter, _key: &[u8], meta: &WebDeploymentInfo) -> bool {
    match filter {
        WebDeploymentFilter::Owner(owner) => meta.owner == *owner,
    }
}
