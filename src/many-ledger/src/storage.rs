use {
    crate::error,
    crate::migration::{
        hash::HASH_MIGRATION, tokens::TOKEN_MIGRATION, LedgerMigrations, MIGRATIONS,
    },
    crate::storage::{account::ACCOUNT_SUBRESOURCE_ID_ROOT, event::HEIGHT_EVENTID_SHIFT},
    derive_more::{From, TryInto},
    many_error::{ManyError, ManyErrorCode},
    many_identity::{Address, MAX_SUBRESOURCE_ID},
    many_migration::{MigrationConfig, MigrationSet},
    many_modules::events::EventId,
    many_types::ledger::Symbol,
    many_types::Timestamp,
    merk_v2::rocksdb::{DBIterator, IteratorMode, ReadOptions},
    merk_v2::Hash,
    std::collections::{BTreeMap, BTreeSet},
    std::path::{Path, PathBuf},
};

mod abci;
pub mod account;
pub mod data;
pub mod event;
pub(crate) mod idstore;
pub mod iterator;
mod ledger;
mod ledger_commands;
pub mod ledger_mintburn;
pub mod ledger_tokens;
mod migrations;
pub mod multisig;

pub const SYMBOLS_ROOT: &str = "/config/symbols";
pub const IDENTITY_ROOT: &str = "/config/identity";
pub const HEIGHT_ROOT: &str = "/height";

pub(super) fn key_for_account_balance(id: &Address, symbol: &Symbol) -> Vec<u8> {
    format!("/balances/{id}/{symbol}").into_bytes()
}

pub(super) fn key_for_subresource_counter(id: &Address, token_migration_active: bool) -> Vec<u8> {
    if token_migration_active {
        format!("/config/subresource_counter/{id}").into_bytes()
    } else {
        // The only subresource counter prior to the token migration is the account subresource
        ACCOUNT_SUBRESOURCE_ID_ROOT.into()
    }
}

pub enum Merk {
    V1(merk_v1::Merk),
    V2(merk_v2::Merk),
}

#[derive(strum::Display, Debug, From, TryInto)]
pub(crate) enum Error {
    V1(merk_v1::Error),
    V2(merk_v2::Error),
}

impl From<Error> for ManyError {
    fn from(error: Error) -> Self {
        match error {
            Error::V1(error) => ManyError::new(
                ManyErrorCode::Unknown,
                Some(error.to_string()),
                BTreeMap::new(),
            ),
            Error::V2(error) => ManyError::new(
                ManyErrorCode::Unknown,
                Some(error.to_string()),
                BTreeMap::new(),
            ),
        }
    }
}

#[derive(Debug, From, TryInto)]
pub(crate) enum Operation {
    V1(merk_v1::Op),
    V2(merk_v2::Op),
}

#[derive(From, TryInto)]
enum Query {
    V1(merk_v1::proofs::query::Query),
    V2(merk_v2::proofs::query::Query),
}

impl Merk {
    fn open_v1<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        merk_v1::Merk::open(path).map(Self::V1).map_err(Into::into)
    }

    pub(crate) fn open_v2<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        merk_v2::Merk::open(path).map(Self::V2).map_err(Into::into)
    }

    pub(crate) fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        match self {
            Merk::V1(merk) => merk.get(key).map_err(Into::into),
            Merk::V2(merk) => merk.get(key).map_err(Into::into),
        }
    }

    pub(crate) fn apply(&mut self, batch: &[(Vec<u8>, Operation)]) -> Result<(), Error> {
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

    pub(crate) fn commit(&mut self, aux: &[(Vec<u8>, Operation)]) -> Result<(), Error> {
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

    pub(crate) fn iter_opt(&self, mode: IteratorMode, readopts: ReadOptions) -> DBIterator {
        match self {
            Merk::V1(merk) => merk.iter_opt(mode, readopts),
            Merk::V2(merk) => merk.iter_opt(mode, readopts),
        }
    }

    fn root_hash(&self) -> Hash {
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

// Merk object storage is organized as a forest, or a collection of trees.
// The different versions necessarily produce different iterator types,
// which is why the following methods for generating them cannot be combined.

pub(crate) fn v1_forest<'a>(
    merk: &'a merk_v1::Merk,
    iterator_mode: IteratorMode,
    read_options: ReadOptions,
) -> impl Iterator<Item = Result<(Vec<u8>, merk_v1::tree::Tree), merk_v1::rocksdb::Error>> + 'a {
    merk.iter_opt(iterator_mode, read_options)
        .map(|key_value_pair| {
            key_value_pair.map(|(key, value)| {
                (
                    key.clone().into(),
                    merk_v1::tree::Tree::decode(key.to_vec(), value.as_ref()),
                )
            })
        })
}

pub(crate) fn v2_forest<'a>(
    merk: &'a merk_v2::Merk,
    iterator_mode: IteratorMode,
    read_options: ReadOptions,
) -> impl Iterator<Item = Result<(Vec<u8>, merk_v2::tree::Tree), merk_v2::rocksdb::Error>> + 'a {
    merk.iter_opt(iterator_mode, read_options)
        .map(|key_value_pair| {
            key_value_pair.map(|(key, value)| {
                (
                    key.clone().into(),
                    merk_v2::tree::Tree::decode(key.to_vec(), value.as_ref()),
                )
            })
        })
}

pub type InnerStorage = Merk;

pub struct LedgerStorage {
    persistent_store: InnerStorage,

    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,

    latest_tid: EventId,

    current_time: Option<Timestamp>,
    current_hash: Option<Vec<u8>>,

    migrations: LedgerMigrations,
    path: PathBuf,
}

impl LedgerStorage {
    #[cfg(feature = "balance_testing")]
    pub(crate) fn set_balance_only_for_testing(
        &mut self,
        account: Address,
        amount: u64,
        symbol: Address,
    ) -> Result<(), ManyError> {
        assert!(self.get_symbols()?.contains(&symbol));
        // Make sure we don't run this function when the store has started.
        assert_eq!(self.current_hash, None);

        let key = key_for_account_balance(&account, &symbol);
        let amount = many_types::ledger::TokenAmount::from(amount);

        self.persistent_store.apply(&[(
            key,
            match self.persistent_store {
                InnerStorage::V1(_) => Operation::from(merk_v1::Op::Put(amount.to_vec())),
                InnerStorage::V2(_) => Operation::from(merk_v2::Op::Put(amount.to_vec())),
            },
        )])?;

        // Always commit to the store. In blockchain mode this will fail.
        self.persistent_store
            .commit(&[])
            .map_err(error::storage_commit_failed)
            .map(|_| ())
    }
}

impl std::fmt::Debug for LedgerStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("LedgerStorage")
            .field("migrations", &self.migrations)
            .finish()
    }
}

impl LedgerStorage {
    #[inline]
    pub fn set_time(&mut self, time: Timestamp) {
        self.current_time = Some(time);
    }
    #[inline]
    pub fn now(&self) -> Timestamp {
        self.current_time.unwrap_or_else(Timestamp::now)
    }

    pub fn migrations(&self) -> &LedgerMigrations {
        &self.migrations
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

    pub fn load<P: AsRef<Path>>(
        persistent_path: P,
        blockchain: bool,
        migration_config: Option<MigrationConfig>,
    ) -> Result<Self, ManyError> {
        let path = persistent_path.as_ref().to_owned();
        let height = InnerStorage::open_v1(path.clone())
            .or_else(|_| InnerStorage::open_v2(path.clone()))
            .map_err(error::storage_open_failed)?
            .get(HEIGHT_ROOT.as_bytes())?
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            });
        let migrations = migration_config
            .map_or_else(MigrationSet::empty, |config| {
                LedgerMigrations::load(&MIGRATIONS, config, height)
            })
            .map_err(error::unable_to_load_migrations)?;
        let persistent_store = if migrations.is_active(&HASH_MIGRATION) {
            InnerStorage::open_v2(path.clone())
        } else {
            InnerStorage::open_v1(path.clone())
        }
        .map_err(error::storage_open_failed)?;

        let height = persistent_store
            .get(HEIGHT_ROOT.as_bytes())
            .map_err(error::storage_get_failed)?
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            });

        // The call to `saturating_sub()` is required to fix
        // https://github.com/liftedinit/many-framework/issues/289
        //
        // The `commit()` function computes the `latest_tid` using the previous height while
        // the following line computes the `latest_tid` using the current height.
        //
        // The discrepancy will lead to an application hash mismatch if the block following the `load()` contains
        // a transaction.
        let latest_tid = EventId::from(height.saturating_sub(1) << HEIGHT_EVENTID_SHIFT);

        Ok(Self {
            persistent_store,
            blockchain,
            latest_tid,
            current_time: None,
            current_hash: None,
            migrations,
            path,
        })
    }

    pub fn new<P: AsRef<Path>>(persistent_path: P, blockchain: bool) -> Result<Self, ManyError> {
        let path = persistent_path.as_ref().to_owned();
        let persistent_store = InnerStorage::open_v1(path.clone())
            .or_else(|_| InnerStorage::open_v2(path.clone()))
            .map_err(ManyError::unknown)?; // TODO: Custom error

        Ok(Self {
            persistent_store,
            blockchain,
            latest_tid: EventId::from(vec![0]),
            current_time: None,
            current_hash: None,
            migrations: MigrationSet::empty().map_err(ManyError::unknown)?, // TODO: Custom error
            path,
        })
    }

    pub fn build(mut self) -> Result<Self, ManyError> {
        self.persistent_store
            .commit(&[])
            .map_err(error::storage_commit_failed)
            .map(|_| self)
    }

    /// Kept for backward compatibility
    pub fn get_symbols_and_tickers(&self) -> Result<BTreeMap<Symbol, String>, ManyError> {
        minicbor::decode::<BTreeMap<Symbol, String>>(
            &self
                .persistent_store
                .get(SYMBOLS_ROOT.as_bytes())
                .map_err(error::storage_get_failed)?
                .ok_or_else(|| error::storage_key_not_found(SYMBOLS_ROOT))?,
        )
        .map_err(ManyError::deserialization_error)
    }

    /// Fetch symbols from `/config/symbols/{symbol}` iif "Token Migration" is enabled
    ///     No CBOR decoding needed.
    /// Else symbols are fetched using the legacy method via `get_symbols_and_tickers()`
    pub fn get_symbols(&self) -> Result<BTreeSet<Symbol>, ManyError> {
        Ok(if self.migrations.is_active(&TOKEN_MIGRATION) {
            self._get_symbols()?
        } else {
            self.get_symbols_and_tickers()?.keys().cloned().collect()
        })
    }

    fn inc_height(&mut self) -> Result<u64, ManyError> {
        let current_height = self.get_height()?;
        self.persistent_store
            .apply(&[(
                HEIGHT_ROOT.as_bytes().to_vec(),
                match self.persistent_store {
                    InnerStorage::V1(_) => Operation::from(merk_v1::Op::Put(
                        (current_height + 1).to_be_bytes().to_vec(),
                    )),
                    InnerStorage::V2(_) => Operation::from(merk_v2::Op::Put(
                        (current_height + 1).to_be_bytes().to_vec(),
                    )),
                },
            )])
            .map_err(Into::into)
            .map(|_| current_height)
    }

    /// Return the current height of the blockchain.
    /// The current height correspond to finished, committed blocks.
    pub fn get_height(&self) -> Result<u64, ManyError> {
        Ok(self
            .persistent_store
            .get(HEIGHT_ROOT.as_bytes())
            .map_err(error::storage_get_failed)?
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            }))
    }

    pub fn hash(&self) -> Vec<u8> {
        self.current_hash
            .as_ref()
            .map_or_else(|| self.persistent_store.root_hash().to_vec(), |x| x.clone())
    }

    /// Get the identity stored at a given DB key
    pub fn get_identity(&self, identity_root: &str) -> Result<Address, ManyError> {
        Address::from_bytes(
            &self
                .persistent_store
                .get(identity_root.as_bytes())
                .map_err(error::storage_get_failed)?
                .ok_or_else(|| error::storage_key_not_found(identity_root))?,
        )
    }

    /// Generate the next subresource from the given identity and counter DB keys.
    /// Uses the server identity to generate the subresource if the given address is not found in the DB.
    pub(crate) fn get_next_subresource(
        &mut self,
        identity_root: &str,
    ) -> Result<(Address, impl IntoIterator<Item = Vec<u8>>), ManyError> {
        let subresource_identity = self
            .persistent_store
            .get(identity_root.as_bytes())
            .map_err(error::storage_get_failed)?
            .map_or(self.get_identity(IDENTITY_ROOT), |bytes| {
                Address::from_bytes(&bytes)
            })?;
        let mut current_id = self.get_subresource_counter(&subresource_identity)?;
        // The last subresource ID we can use is == MAX_SUBRESOURCE_ID
        // Check if the next counter is over the maximum
        if current_id > MAX_SUBRESOURCE_ID {
            return Err(error::subresource_exhausted(subresource_identity));
        }
        let symbols = self.get_symbols()?;
        let mut next_subresource = subresource_identity.with_subresource_id(current_id)?;

        while symbols.contains(&next_subresource) {
            current_id += 1;
            // Check if the next counter is over the maximum
            if current_id > MAX_SUBRESOURCE_ID {
                return Err(error::subresource_exhausted(subresource_identity));
            }
            next_subresource = subresource_identity.with_subresource_id(current_id)?;
        }

        let key_for_subresource = key_for_subresource_counter(
            &subresource_identity,
            self.migrations.is_active(&TOKEN_MIGRATION),
        );

        self.persistent_store.apply(&[(
            key_for_subresource.clone(),
            match self.persistent_store {
                InnerStorage::V1(_) => {
                    merk_v1::Op::Put((current_id + 1).to_be_bytes().to_vec()).into()
                }
                InnerStorage::V2(_) => {
                    merk_v2::Op::Put((current_id + 1).to_be_bytes().to_vec()).into()
                }
            },
        )])?;
        let mut keys = vec![key_for_subresource];

        self.persistent_store
            .get((*identity_root).as_bytes())
            .map_err(error::storage_get_failed)?
            .map_or(
                {
                    keys.push(IDENTITY_ROOT.into());
                    self.get_identity(IDENTITY_ROOT)
                },
                |bytes| {
                    keys.push(identity_root.as_bytes().to_vec());
                    Address::from_bytes(&bytes)
                },
            )?
            .with_subresource_id(current_id)
            .map(|address| (address, keys))
    }

    /// Get the subresource counter from the given DB key.
    /// Returns 0 if the key is not found in the DB
    fn get_subresource_counter(&self, id: &Address) -> Result<u32, ManyError> {
        Ok(self
            .persistent_store
            .get(&key_for_subresource_counter(
                id,
                self.migrations.is_active(&TOKEN_MIGRATION),
            ))
            .map_err(error::storage_get_failed)?
            .map_or(0, |x| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(x.as_slice());
                u32::from_be_bytes(bytes)
            }))
    }

    pub fn block_hotfix<
        T: minicbor::Encode<()>,
        C: for<'a> minicbor::Decode<'a, ()>,
        F: FnOnce() -> T,
    >(
        &mut self,
        name: &str,
        data: F,
    ) -> Result<Option<C>, ManyError> {
        let data_enc = minicbor::to_vec(data()).map_err(ManyError::serialization_error)?;

        if let Some(data) = self
            .migrations
            .hotfix(name, &data_enc, self.get_height()? + 1)?
        {
            minicbor::decode(&data)
                .map_err(ManyError::deserialization_error)
                .map(Some)
        } else {
            Ok(None)
        }
    }
}
