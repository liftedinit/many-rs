use crate::error;
use crate::storage::iterator::ComputeIterator;
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::AbciCommitInfo;
use many_modules::events::EventId;
use many_types::compute::{ComputeStatus, DeploymentMeta};
use many_types::{SortOrder, Timestamp};
use merk::{BatchEntry, Op};
use std::path::Path;

pub mod iterator;

pub struct ComputeStorage {
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

impl std::fmt::Debug for ComputeStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComputeStorage").finish()
    }
}

impl ComputeStorage {
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

    pub fn add_deployment(
        &mut self,
        sender: &Address,
        meta: &DeploymentMeta,
    ) -> Result<(), ManyError> {
        let dseq = meta.dseq;
        self.persistent_store
            .apply(&[(
                format!("/deploy/{sender}/{dseq}").into_bytes(),
                Op::Put(minicbor::to_vec(meta).map_err(ManyError::serialization_error)?),
            )])
            .map_err(error::storage_apply_failed)?;

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(())
    }

    pub fn has(&self, owner: &Address, dseq: u64) -> Result<bool, ManyError> {
        Ok(self
            .persistent_store
            .get(format!("/deploy/{owner}/{dseq}").as_bytes())
            .map_err(error::storage_get_failed)?
            .is_some())
    }

    pub fn remove_deployment(&mut self, sender: &Address, dseq: u64) -> Result<(), ManyError> {
        let mut meta: DeploymentMeta = minicbor::decode(
            &self
                .persistent_store
                .get(format!("/deploy/{sender}/{dseq}").as_bytes())
                .map_err(error::storage_get_failed)?
                .ok_or(ManyError::unknown("Option is null"))?,
        )
        .map_err(ManyError::deserialization_error)?; // TODO: better error

        meta.status = ComputeStatus::Closed;
        meta.meta = None;

        self.persistent_store
            .apply(&[(
                format!("/deploy/{sender}/{dseq}").into_bytes(),
                Op::Put(minicbor::to_vec(meta).map_err(ManyError::serialization_error)?),
            )])
            .map_err(error::storage_apply_failed)?;

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }

        Ok(())
    }

    pub fn list_deployment(
        &self,
        order: Option<SortOrder>,
        owner: Option<Address>,
    ) -> Result<Vec<DeploymentMeta>, ManyError> {
        ComputeIterator::all_dseq(&self.persistent_store, order, owner)
            .map(|item| {
                let (_, v) = item.map_err(error::storage_get_failed)?;
                let meta: DeploymentMeta =
                    minicbor::decode(&v).map_err(ManyError::deserialization_error)?;
                Ok(meta)
            })
            .collect::<Result<Vec<_>, _>>()
    }
}
