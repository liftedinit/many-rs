use crate::error;
use crate::storage::iterator::WebIterator;
use base64::engine::general_purpose;
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::AbciCommitInfo;
use many_modules::events::{EventId, EventInfo};
use many_types::web::{WebDeploymentFilter, WebDeploymentInfo};
use many_types::{Memo, SortOrder, Timestamp};
use merk::{BatchEntry, Op};
use std::fs;
use std::io::Write;
use std::path::Path;
use tracing::trace;
use walkdir::{DirEntry, WalkDir};

pub mod events;
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
    let domain = crate::DOMAIN.get_or_init(|| "localhost:8880".to_string());
    format!("https://{site_name}-{owner}.{domain}")
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
    #[inline]
    pub fn now(&self) -> Timestamp {
        self.current_time.unwrap_or_else(Timestamp::now)
    }

    #[inline]
    fn maybe_commit(&mut self) -> Result<(), ManyError> {
        if !self.blockchain {
            self.commit_storage()
        } else {
            Ok(())
        }
    }

    #[inline]
    fn commit_storage(&mut self) -> Result<(), ManyError> {
        self.persistent_store
            .commit(&[])
            .map_err(error::storage_commit_failed)
    }

    pub fn load<P: AsRef<Path>>(persistent_path: P, blockchain: bool) -> Result<Self, ManyError> {
        let persistent_store =
            merk::Merk::open(persistent_path).map_err(error::unable_to_open_storage)?;

        let next_subresource = persistent_store
            .get(b"/config/subresource_id")
            .map_err(error::storage_get_failed)?
            .map_or(0, |x| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(x.as_slice());
                u32::from_be_bytes(bytes)
            });

        let root_identity: Address = Address::from_bytes(
            &persistent_store
                .get(b"/config/identity")
                .map_err(error::storage_get_failed)?
                .ok_or(error::key_not_found("/config/identity"))?,
        )
        .map_err(ManyError::deserialization_error)?;

        let latest_event_id = minicbor::decode(
            &persistent_store
                .get(b"/latest_event_id")
                .map_err(error::storage_get_failed)?
                .ok_or(error::key_not_found("/latest_event_id"))?,
        )
        .map_err(ManyError::deserialization_error)?;

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
    ) -> Result<Self, ManyError> {
        let mut persistent_store =
            merk::Merk::open(persistent_path).map_err(error::unable_to_open_storage)?;

        let batch: Vec<BatchEntry> =
            vec![(b"/config/identity".to_vec(), Op::Put(identity.to_vec()))];

        persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        let latest_event_id = EventId::from(vec![0]);
        persistent_store
            .apply(&[(
                b"/latest_event_id".to_vec(),
                Op::Put(
                    minicbor::to_vec(&latest_event_id).map_err(ManyError::serialization_error)?,
                ),
            )])
            .map_err(error::storage_apply_failed)?;

        persistent_store
            .commit(&[])
            .map_err(error::storage_commit_failed)?;

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

    pub fn site_exists(&self, owner: &Address, site_name: &str) -> Result<bool, ManyError> {
        let key_meta = key_for_website_meta(owner, site_name);
        let key_index = key_for_website_file(owner, site_name, "index.html");
        let meta_exists = self
            .get(&key_meta)
            .map_err(error::storage_get_failed)?
            .is_some();
        let index_exists = self
            .get(key_index.as_bytes())
            .map_err(error::storage_get_failed)?
            .is_some();
        Ok(meta_exists && index_exists)
    }

    fn inc_height(&mut self) -> Result<u64, ManyError> {
        let current_height = self.get_height()?;
        self.persistent_store
            .apply(&[(
                b"/height".to_vec(),
                Op::Put((current_height + 1).to_be_bytes().to_vec()),
            )])
            .map_err(error::storage_apply_failed)?;
        Ok(current_height)
    }

    pub fn get_height(&self) -> Result<u64, ManyError> {
        self.persistent_store
            .get(b"/height")
            .map_err(error::storage_get_failed)?
            .map_or(Ok(0u64), |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                Ok(u64::from_be_bytes(bytes))
            })
    }

    pub fn commit(&mut self) -> Result<AbciCommitInfo, ManyError> {
        let _ = self.inc_height();
        self.persistent_store
            .apply(&[(
                b"/latest_event_id".to_vec(),
                Op::Put(
                    minicbor::to_vec(&self.latest_event_id)
                        .map_err(ManyError::serialization_error)?,
                ),
            )])
            .map_err(error::storage_apply_failed)?;
        self.persistent_store
            .commit(&[])
            .map_err(error::storage_commit_failed)?;

        let retain_height = 0;
        let hash = self.persistent_store.root_hash().to_vec();
        self.current_hash = Some(hash.clone());

        Ok(AbciCommitInfo {
            retain_height,
            hash: hash.into(),
        })
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

    fn _store_website(
        &mut self,
        owner: &Address,
        site_name: &str,
        site_description: &Option<String>,
        path: impl AsRef<Path>,
        domain: &Option<String>,
    ) -> Result<Vec<BatchEntry>, ManyError> {
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
                key_for_website_file(owner, site_name, file_path)
            );
            batch.push((
                key_for_website_file(owner, site_name, file_path).into_bytes(),
                Op::Put(data),
            ));
        }

        let url = url_for_website(owner, site_name);

        trace!("Adding website meta to batch");
        tracing::debug!(
            "Key: {}",
            String::from_utf8(key_for_website_meta(owner, site_name)).unwrap()
        );
        batch.push((
            key_for_website_meta(owner, site_name),
            Op::Put(
                minicbor::to_vec(WebDeploymentInfo {
                    owner: *owner,
                    site_name: site_name.to_owned(),
                    site_description: site_description.clone(),
                    url: Some(url),
                    domain: domain.to_owned(),
                })
                .map_err(ManyError::serialization_error)?,
            ),
        ));

        trace!("Sorting batch");
        batch.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

        Ok(batch)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store_website(
        &mut self,
        owner: &Address,
        site_name: String,
        site_description: Option<String>,
        memo: Option<Memo>,
        source_hash: String,
        path: impl AsRef<Path>,
        domain: Option<String>,
    ) -> Result<(), ManyError> {
        let batch = self._store_website(owner, &site_name, &site_description, path, &domain)?;

        trace!("Applying batch");
        self.persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        self.log_event(EventInfo::WebDeploy {
            owner: *owner,
            site_name: site_name.clone(),
            site_description,
            source_hash,
            memo,
            domain,
        })?;

        self.maybe_commit()?;

        Ok(())
    }

    fn _remove_website(
        &self,
        owner: &Address,
        site_name: &String,
    ) -> Result<Vec<BatchEntry>, ManyError> {
        trace!("Removing website {}", site_name);
        let it = WebIterator::website_files(&self.persistent_store, owner, &site_name);

        let mut batch: Vec<BatchEntry> = Vec::new();

        // Remove each file of the website
        for item in it {
            let (key, _) = item.map_err(error::storage_get_failed)?;
            batch.push((key.to_vec(), Op::Delete));
        }

        trace!("Removing website meta");
        tracing::debug!(
            "Key: {}",
            String::from_utf8(key_for_website_meta(owner, site_name)).unwrap()
        );
        batch.push((key_for_website_meta(owner, site_name), Op::Delete));

        Ok(batch)
    }

    pub fn remove_website(
        &mut self,
        owner: &Address,
        site_name: String,
        memo: Option<Memo>,
    ) -> Result<(), ManyError> {
        let batch = self._remove_website(owner, &site_name)?;

        self.persistent_store
            .apply(&batch)
            .map_err(error::storage_apply_failed)?;

        self.log_event(EventInfo::WebRemove {
            owner: *owner,
            site_name,
            memo,
        })?;

        self.maybe_commit()?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_website(
        &mut self,
        owner: &Address,
        site_name: String,
        site_description: Option<String>,
        memo: Option<Memo>,
        source_hash: String,
        path: impl AsRef<Path>,
        domain: Option<String>,
    ) -> Result<(), ManyError> {
        trace!("Removing website prior to update");
        let batch_r = self._remove_website(owner, &site_name)?;

        trace!("Storing updated website");
        let batch_s = self._store_website(owner, &site_name, &site_description, path, &domain)?;

        // `merk` doesn't support applying `b1` and `b2` where
        // - `b1` contains a `Delete` operation and
        // - `b2` contains a `Put` operation
        // over the same key.
        //
        // E.g.
        // ```
        // let b1 = vec![(b"key".to_vec(), Op::Delete)];
        // storage.apply(&b1)?;
        // let b2 = vec![(b"key".to_vec(), Op::Put(b"value".to_vec()))];
        // storage.apply(&b2)?;
        // storage.commit(&[])?;
        // ```
        //
        // The above doesn't work. The `Put` operation in `b2` is ignored.

        trace!("Combining batches");
        // The "website storing" batch is first so that if there's any duplicated keys the "remove website"
        // operation gets removed from the batch by the `dedup` operation.
        let mut combined: Vec<_> = batch_s.into_iter().chain(batch_r).collect();

        trace!("Sorting batch");
        // Sort the combined batches by key. Any duplicated keys will be next to each other.
        combined.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

        trace!("Deduping batch");
        // Remove duplicate keys. The first item of the duplicated keys will be kept.
        combined.dedup_by(|(k1, _), (k2, _)| k1 == k2);

        self.persistent_store
            .apply(&combined)
            .map_err(error::storage_apply_failed)?;

        self.log_event(EventInfo::WebUpdate {
            owner: *owner,
            site_name,
            site_description,
            source_hash,
            memo,
            domain,
        })?;
        self.maybe_commit()?;

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
