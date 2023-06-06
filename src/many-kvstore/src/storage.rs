use crate::module::{KvStoreMetadata, KvStoreMetadataWrapper};
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::AbciCommitInfo;
use many_modules::events::EventInfo;
use many_types::{Either, ProofOperation, Timestamp};
use merk::{
    proofs::{
        Decoder,
        Node::{Hash, KVHash, KV},
        Query,
    },
    BatchEntry, Op,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

mod account;
mod event;

use crate::error;
use event::EventId;

const KVSTORE_ROOT: &[u8] = b"s";
const KVSTORE_ACL_ROOT: &[u8] = b"a";

#[derive(Serialize, Deserialize, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct Key {
    #[serde(with = "hex::serde")]
    key: Vec<u8>,
}

pub type AclMap = BTreeMap<Key, KvStoreMetadataWrapper>;

pub struct KvStoreStorage {
    persistent_store: merk::Merk,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,

    latest_event_id: EventId,
    current_time: Option<Timestamp>,
    current_hash: Option<Vec<u8>>,
    next_subresource: u32,
    root_identity: Address,
}

impl std::fmt::Debug for KvStoreStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KvStoreStorage").finish()
    }
}

impl KvStoreStorage {
    #[inline]
    pub fn set_time(&mut self, time: Timestamp) {
        self.current_time = Some(time);
    }
    #[inline]
    pub fn now(&self) -> Timestamp {
        self.current_time.unwrap_or_else(Timestamp::now)
    }

    pub fn new_subresource_id(&mut self) -> Result<(Address, Vec<u8>), ManyError> {
        let current_id = self.next_subresource;
        self.next_subresource += 1;
        let key = b"/config/subresource_id".to_vec();
        self.persistent_store
            .apply(&[(
                key.clone(),
                Op::Put(self.next_subresource.to_be_bytes().to_vec()),
            )])
            .map_err(error::storage_apply_failed)?;

        self.root_identity
            .with_subresource_id(current_id)
            .map(|address| (address, key))
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
        acl: AclMap,
        identity: Address,
        persistent_path: P,
        blockchain: bool,
    ) -> Result<Self, String> {
        let mut persistent_store = merk::Merk::open(persistent_path).map_err(|e| e.to_string())?;

        let mut batch: Vec<BatchEntry> = Vec::new();

        batch.push((b"/config/identity".to_vec(), Op::Put(identity.to_vec())));

        // Initialize DB with ACL
        for (k, v) in acl.into_iter() {
            batch.push((
                vec![KVSTORE_ACL_ROOT.to_vec(), k.key.to_vec()].concat(),
                Op::Put(minicbor::to_vec(v).map_err(|e| e.to_string())?),
            ));
        }

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

    fn _get(&self, key: &[u8], prefix: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        self.persistent_store
            .get(&vec![prefix.to_vec(), key.to_vec()].concat())
            .map_err(|e| ManyError::unknown(e.to_string()))
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        if let Some(cbor) = self._get(key, KVSTORE_ACL_ROOT)? {
            let meta: KvStoreMetadata = minicbor::decode(&cbor)
                .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

            if let Some(either) = meta.disabled {
                match either {
                    Either::Left(false) => {}
                    _ => return Err(error::key_disabled()),
                }
            }
        }
        self._get(key, KVSTORE_ROOT)
    }

    pub fn get_metadata(&self, key: &[u8]) -> Result<Option<Vec<u8>>, ManyError> {
        self._get(key, KVSTORE_ACL_ROOT)
    }

    pub fn put(
        &mut self,
        meta: &KvStoreMetadata,
        key: &[u8],
        value: Vec<u8>,
    ) -> Result<(), ManyError> {
        self.persistent_store
            .apply(&[
                (
                    vec![KVSTORE_ACL_ROOT.to_vec(), key.to_vec()].concat(),
                    Op::Put(
                        minicbor::to_vec(meta)
                            .map_err(|e| ManyError::serialization_error(e.to_string()))?,
                    ),
                ),
                (
                    vec![KVSTORE_ROOT.to_vec(), key.to_vec()].concat(),
                    Op::Put(value.clone()),
                ),
            ])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        self.log_event(EventInfo::KvStorePut {
            key: key.to_vec().into(),
            value: value.into(),
            owner: meta.owner,
        });

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
        Ok(())
    }

    pub fn disable(&mut self, meta: &KvStoreMetadata, key: &[u8]) -> Result<(), ManyError> {
        self.persistent_store
            .apply(&[(
                vec![KVSTORE_ACL_ROOT.to_vec(), key.to_vec()].concat(),
                Op::Put(
                    minicbor::to_vec(meta)
                        .map_err(|e| ManyError::serialization_error(e.to_string()))?,
                ),
            )])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        let reason = if let Some(disabled) = &meta.disabled {
            match disabled {
                Either::Right(reason) => Some(reason),
                _ => None,
            }
        } else {
            None
        };

        self.log_event(EventInfo::KvStoreDisable {
            key: key.to_vec().into(),
            reason: reason.cloned(),
        });

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
        Ok(())
    }

    pub fn transfer(
        &mut self,
        key: &[u8],
        previous_owner: Address,
        meta: KvStoreMetadata,
    ) -> Result<(), ManyError> {
        let new_owner = meta.owner;
        self.persistent_store
            .apply(&[(
                vec![KVSTORE_ACL_ROOT.to_vec(), key.to_vec()].concat(),
                Op::Put(
                    minicbor::to_vec(meta)
                        .map_err(|e| ManyError::serialization_error(e.to_string()))?,
                ),
            )])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        self.log_event(EventInfo::KvStoreTransfer {
            key: key.to_vec().into(),
            owner: previous_owner,
            new_owner,
        });

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
        Ok(())
    }

    pub fn prove_state(
        &self,
        context: impl AsRef<many_protocol::context::Context>,
        keys: impl IntoIterator<Item = Vec<u8>>,
    ) -> Result<(), ManyError> {
        use merk::proofs::Op;
        context.as_ref().prove(|| {
            self.persistent_store
                .prove({
                    let mut query = Query::new();
                    keys.into_iter().for_each(|key| query.insert_key(key));
                    query
                })
                .and_then(|proof| {
                    Decoder::new(proof.as_slice())
                        .map(|fallible_operation| {
                            fallible_operation.map(|operation| match operation {
                                Op::Child => ProofOperation::Child,
                                Op::Parent => ProofOperation::Parent,
                                Op::Push(Hash(hash)) => ProofOperation::NodeHash(hash.to_vec()),
                                Op::Push(KV(key, value)) => {
                                    ProofOperation::KeyValuePair(key.into(), value.into())
                                }
                                Op::Push(KVHash(hash)) => {
                                    ProofOperation::KeyValueHash(hash.to_vec())
                                }
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .map_err(|error| ManyError::unknown(error.to_string()))
        })
    }
}
