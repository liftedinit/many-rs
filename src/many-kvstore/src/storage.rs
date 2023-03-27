use crate::module::{KvStoreMetadata, KvStoreMetadataWrapper};
use derive_more::{From, TryInto};
use many_error::{ManyError, ManyErrorCode};
use many_identity::Address;
use many_modules::abci_backend::AbciCommitInfo;
use many_modules::events::EventInfo;
use many_types::{Either, ProofOperation, Timestamp};
use merk_v1::rocksdb::{DBIterator, IteratorMode, ReadOptions};
use merk_v1::{
    proofs::{
        query::QueryItem,
        Decoder,
        Node::{Hash, KVHash, KV},
    },
    Hash as MerkHash, Op,
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

pub enum Merk {
    V1(merk_v1::Merk),
    #[allow(dead_code)]
    V2(merk_v2::Merk),
}

#[derive(strum::Display, Debug, From, TryInto)]
enum Error {
    V1(merk_v1::Error),
    V2(merk_v2::Error),
}

impl From<Error> for ManyError {
    fn from(error: Error) -> Self {
        match error {
            Error::V1(error) => ManyError::new(ManyErrorCode::Unknown, Some(error.to_string()), BTreeMap::new()),
            Error::V2(error) => ManyError::new(ManyErrorCode::Unknown, Some(error.to_string()), BTreeMap::new()),
        }
    }
}

#[derive(Debug, From, TryInto)]
enum Operation {
    V1(merk_v1::Op),
    V2(merk_v2::Op),
}

#[derive(From, TryInto)]
enum Query {
    V1(merk_v1::proofs::query::Query),
    V2(merk_v2::proofs::query::Query),
}

impl Merk {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        merk_v1::Merk::open(path).map(Self::V1).map_err(Into::into)
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        match self {
            Merk::V1(merk) => merk.get(key).map_err(Into::into),
            Merk::V2(merk) => merk.get(key).map_err(Into::into),
        }
    }

    fn apply(&mut self, batch: &[(Vec<u8>, Operation)]) -> Result<(), Error> {
        match self {
            Merk::V1(merk) => merk
                .apply(
                    batch
                        .iter()
                        .filter_map(|(key, op)| match op {
                            Operation::V1(operation) => Some((
                                key.clone(),
                                match operation {
                                    merk_v1::Op::Put(value) => merk_v1::Op::Put(value.clone()),
                                    merk_v1::Op::Delete => merk_v1::Op::Delete,
                                },
                            )),
                            Operation::V2(_) => None,
                        })
                        .collect::<Vec<_>>()
                        .as_slice(),
                )
                .map_err(Into::into),
            Merk::V2(merk) => merk
                .apply(
                    batch
                        .iter()
                        .filter_map(|(key, op)| match op {
                            Operation::V1(_) => None,
                            Operation::V2(operation) => Some((
                                key.clone(),
                                match operation {
                                    merk_v2::Op::Put(value) => merk_v2::Op::Put(value.clone()),
                                    merk_v2::Op::Delete => merk_v2::Op::Delete,
                                },
                            )),
                        })
                        .collect::<Vec<_>>()
                        .as_slice(),
                )
                .map_err(Into::into),
        }
    }

    fn commit(&mut self, aux: &[(Vec<u8>, Operation)]) -> Result<(), Error> {
        match self {
            Merk::V1(merk) => merk
                .commit(
                    aux.iter()
                        .filter_map(|(key, op)| match op {
                            Operation::V1(operation) => Some((
                                key.clone(),
                                match operation {
                                    merk_v1::Op::Put(value) => merk_v1::Op::Put(value.clone()),
                                    merk_v1::Op::Delete => merk_v1::Op::Delete,
                                },
                            )),
                            Operation::V2(_) => None,
                        })
                        .collect::<Vec<_>>()
                        .as_slice(),
                )
                .map_err(Into::into),
            Merk::V2(merk) => merk
                .commit(
                    aux.iter()
                        .filter_map(|(key, op)| match op {
                            Operation::V1(_) => None,
                            Operation::V2(operation) => Some((
                                key.clone(),
                                match operation {
                                    merk_v2::Op::Put(value) => merk_v2::Op::Put(value.clone()),
                                    merk_v2::Op::Delete => merk_v2::Op::Delete,
                                },
                            )),
                        })
                        .collect::<Vec<_>>()
                        .as_slice(),
                )
                .map_err(Into::into),
        }
    }

    fn iter_opt(&self, mode: IteratorMode, readopts: ReadOptions) -> DBIterator {
        match self {
            Merk::V1(merk) => merk.iter_opt(mode, readopts),
            Merk::V2(merk) => merk.iter_opt(mode, readopts),
        }
    }

    fn root_hash(&self) -> MerkHash {
        match self {
            Merk::V1(merk) => merk.root_hash(),
            Merk::V2(merk) => merk.root_hash(),
        }
    }

    fn prove(&self, query: Query) -> Result<Vec<u8>, Error> {
        match self {
            Merk::V1(merk) => match query {
                Query::V1(query) => merk.prove(query).map_err(Into::into),
                Query::V2(_) => Err(merk_v1::Error::Proof(
                    "Wrong version of query submitted for version of proof requested".into(),
                )
                .into()),
            },
            Merk::V2(merk) => match query {
                Query::V1(_) => Err(merk_v2::Error::Proof(
                    "Wrong version of query submitted for version of proof requested".into(),
                )
                .into()),
                Query::V2(query) => merk.prove(query).map_err(Into::into),
            },
        }
    }
}

pub type AclMap = BTreeMap<Key, KvStoreMetadataWrapper>;
pub(crate) type InnerStorage = Merk;

pub struct KvStoreStorage {
    persistent_store: InnerStorage,

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
                Op::Put(self.next_subresource.to_be_bytes().to_vec()).into(),
            )])?;

        self.root_identity
            .with_subresource_id(current_id)
            .map(|address| (address, key))
    }

    pub fn load<P: AsRef<Path>>(persistent_path: P, blockchain: bool) -> Result<Self, String> {
        let persistent_store = InnerStorage::open(persistent_path).map_err(|e| e.to_string())?;

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
        let mut persistent_store =
            InnerStorage::open(persistent_path).map_err(|e| e.to_string())?;

        let mut batch: Vec<(Vec<u8>, Operation)> = Vec::new();

        batch.push((
            b"/config/identity".to_vec(),
            Op::Put(identity.to_vec()).into(),
        ));

        // Initialize DB with ACL
        for (k, v) in acl.into_iter() {
            batch.push((
                vec![KVSTORE_ACL_ROOT.to_vec(), k.key.to_vec()].concat(),
                Op::Put(minicbor::to_vec(v).map_err(|e| e.to_string())?).into(),
            ));
        }

        persistent_store
            .apply(batch.as_slice())
            .map_err(|e| e.to_string())?;

        let latest_event_id = EventId::from(vec![0]);
        persistent_store
            .apply(&[(
                b"/latest_event_id".to_vec(),
                Op::Put(minicbor::to_vec(&latest_event_id).expect("Unable to encode event id"))
                    .into(),
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
                Op::Put((current_height + 1).to_be_bytes().to_vec()).into(),
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
                )
                .into(),
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
                    )
                    .into(),
                ),
                (
                    vec![KVSTORE_ROOT.to_vec(), key.to_vec()].concat(),
                    Op::Put(value.clone()).into(),
                ),
            ])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        self.log_event(EventInfo::KvStorePut {
            key: key.to_vec().into(),
            value: value.into(),
            owner: meta.owner,
        });

        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .map_err(ManyError::unknown)?;
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
                )
                .into(),
            )])
            .map_err(ManyError::unknown)?;

        let reason = if let Some(disabled) = &meta.disabled {
            match disabled {
                Either::Right(reason) => Some(reason),
                Either::Left(_) => None,
            }
        } else {
            None
        };

        self.log_event(EventInfo::KvStoreDisable {
            key: key.to_vec().into(),
            reason: reason.cloned(),
        });

        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .map_err(ManyError::unknown)?;
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
                )
                .into(),
            )])
            .map_err(ManyError::unknown)?;

        self.log_event(EventInfo::KvStoreTransfer {
            key: key.to_vec().into(),
            owner: previous_owner,
            new_owner,
        });

        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .map_err(ManyError::unknown)?;
        }
        Ok(())
    }

    pub fn prove_state(
        &self,
        context: impl AsRef<many_protocol::context::Context>,
        keys: impl IntoIterator<Item = Vec<u8>>,
    ) -> Result<(), ManyError> {
        use merk_v1::proofs::Op;
        context.as_ref().prove(|| {
            self.persistent_store
                .prove(
                    merk_v1::proofs::query::Query::from(
                        keys.into_iter().map(QueryItem::Key).collect::<Vec<_>>(),
                    )
                    .into(),
                )
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
                        .map_err(Into::into)
                })
                .map_err(|error| ManyError::unknown(error.to_string()))
        })
    }
}
