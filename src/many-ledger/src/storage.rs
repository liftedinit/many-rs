use crate::error;
use crate::migration::tokens::TOKEN_MIGRATION;
use crate::migration::{LedgerMigrations, MIGRATIONS};
use crate::storage::account::ACCOUNT_SUBRESOURCE_ID_ROOT;
use crate::storage::event::HEIGHT_EVENTID_SHIFT;
use async_channel::{Receiver, Sender, TrySendError};
use many_error::ManyError;
use many_identity::{Address, MAX_SUBRESOURCE_ID};
use many_migration::{MigrationConfig, MigrationSet};
use many_modules::events::EventId;
use many_protocol::context::{Context, ProofResult};
use many_types::ledger::Symbol;
use many_types::{ProofOperation, Timestamp};
use merk::{
    proofs::{
        Decoder,
        Node::{Hash, KVHash, KV},
        Op::{Child, Parent, Push},
        Query,
    },
    Batch, BatchEntry, Error, Op,
};
use std::collections::{BTreeMap, BTreeSet};
use std::{borrow::Cow, path::Path};

mod abci;
pub mod account;
pub mod data;
pub mod event;
mod idstore;
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

pub type InnerStorage = merk::Merk;

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

        self.persistent_store
            .apply(&[(key, Op::Put(amount.to_vec()))])
            .map_err(error::storage_apply_failed)?;

        // Always commit to the store. In blockchain mode this will fail.
        self.persistent_store
            .commit(&[])
            .map_err(error::storage_commit_failed)?;
        Ok(())
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
        let persistent_store =
            InnerStorage::open(persistent_path).map_err(error::storage_open_failed)?;

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
        let migrations = migration_config
            .map_or_else(MigrationSet::empty, |config| {
                LedgerMigrations::load(&MIGRATIONS, config, height)
            })
            .map_err(error::unable_to_load_migrations)?;

        Ok(Self {
            persistent_store,
            blockchain,
            latest_tid,
            current_time: None,
            current_hash: None,
            migrations,
        })
    }

    pub fn new<P: AsRef<Path>>(
        symbols: &BTreeMap<Symbol, String>,
        persistent_path: P,
        identity: Address,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let mut persistent_store =
            InnerStorage::open(persistent_path).map_err(ManyError::unknown)?; // TODO: Custom error

        persistent_store
            .apply(&[
                (
                    IDENTITY_ROOT.as_bytes().to_vec(),
                    Op::Put(identity.to_vec()),
                ),
                (
                    SYMBOLS_ROOT.as_bytes().to_vec(),
                    Op::Put(minicbor::to_vec(symbols).map_err(ManyError::serialization_error)?),
                ),
            ])
            .map_err(error::storage_apply_failed)?;

        // We need to commit, because we need IDENTITY_ROOT to be available for the next steps, if any.
        persistent_store
            .commit(&[])
            .map_err(error::storage_commit_failed)?;

        Ok(Self {
            persistent_store,
            blockchain,
            latest_tid: EventId::from(vec![0]),
            current_time: None,
            current_hash: None,
            migrations: MigrationSet::empty().map_err(ManyError::unknown)?, // TODO: Custom error
        })
    }

    pub fn build(mut self) -> Result<Self, ManyError> {
        self.persistent_store
            .commit(&[])
            .map_err(error::storage_commit_failed).map(|_| self)
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
                Op::Put((current_height + 1).to_be_bytes().to_vec()),
            )])
            .map_err(error::storage_apply_failed)?;
        Ok(current_height)
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
    ) -> Result<Address, ManyError> {
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

        self.persistent_store
            .apply(&[(
                key_for_subresource_counter(
                    &subresource_identity,
                    self.migrations.is_active(&TOKEN_MIGRATION),
                ),
                Op::Put((current_id + 1).to_be_bytes().to_vec()),
            )])
            .map_err(error::storage_apply_failed)?;

        self.persistent_store
            .get(identity_root.as_bytes())
            .map_err(error::storage_get_failed)?
            .map_or(self.get_identity(IDENTITY_ROOT), |bytes| {
                Address::from_bytes(&bytes)
            })?
            .with_subresource_id(current_id)
    }

    /// Get the subresource counter from the given DB key.
    /// Returns 0 if the key is not found in the DB
    fn get_subresource_counter(&self, id: &Address) -> Result<u32, ManyError> {
        self.persistent_store
            .get(&key_for_subresource_counter(
                id,
                self.migrations.is_active(&TOKEN_MIGRATION),
            ))
            .map_err(error::storage_get_failed)?
            .map_or(Ok(0), |x| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(x.as_slice());
                Ok(u32::from_be_bytes(bytes))
            })
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
            let dec_data = minicbor::decode(&data).map_err(ManyError::deserialization_error)?;
            Ok(Some(dec_data))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn underlying_store(&mut self) -> &mut InnerStorage {
        &mut self.persistent_store
    }
}

pub struct ProvingStore<'a> {
    context: &'a Context,
    keys: Vec<Vec<u8>>,
    storage: &'a mut merk::Merk,
    /// When this is true, we do not commit every transactions as they come,
    /// but wait for a `commit` call before committing the batch to the
    /// persistent store.
    blockchain: bool,
    latest_tid: EventId,
    current_time: Option<Timestamp>,
    current_hash: Option<Vec<u8>>,
    migrations: &'a LedgerMigrations,
}

pub enum KeyRetrievalResult {
    Retrieved(Vec<u8>),
    NotFound,
    Error(Error),
}

impl std::fmt::Debug for ProvingStore<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("ProvingStorage")
            .field("migrations", &self.migrations)
            .finish()
    }
}

impl<'a> ProvingStore<'a> {
    pub fn new(context: &'a Context, underlying: &'a mut LedgerStorage) -> Self {
        Self {
            context,
            keys: vec![],
            storage: &mut underlying.persistent_store,
            blockchain: underlying.blockchain,
            latest_tid: underlying.latest_tid.clone(),
            current_time: underlying.current_time,
            current_hash: underlying.current_hash.clone(),
            migrations: &underlying.migrations
        }
    }

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

        self.storage
            .apply(&[(key, Op::Put(amount.to_vec()))])
            .map_err(error::storage_apply_failed)?;

        // Always commit to the store. In blockchain mode this will fail.
        self.storage
            .commit(&[])
            .map_err(error::storage_commit_failed)
    }

    /// Kept for backward compatibility
    pub fn get_symbols_and_tickers(&self) -> Result<BTreeMap<Symbol, String>, ManyError> {
        minicbor::decode::<BTreeMap<Symbol, String>>(
            &self
                .storage
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

    /// Fetch symbols from `/config/symbols/{symbol}`
    ///     No CBOR decoding needed.
    pub(crate) fn _get_symbols(&self) -> Result<BTreeSet<Symbol>, ManyError> {
        use {
            crate::storage::{iterator::LedgerIterator, ledger_tokens::SYMBOLS_ROOT_DASH},
            many_types::SortOrder,
            std::str::FromStr
        };
        let mut symbols = BTreeSet::new();
        let it = LedgerIterator::all_symbols(&self.storage, SortOrder::Indeterminate);
        for item in it {
            let (k, _) = item.map_err(ManyError::unknown)?;
            symbols.insert(Symbol::from_str(
                std::str::from_utf8(&k.as_ref()[SYMBOLS_ROOT_DASH.len()..])
                    .map_err(ManyError::deserialization_error)?, // TODO: We could safely use from_utf8_unchecked() if performance is an issue
            )?);
        }
        Ok(symbols)
    }

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
    fn commit_storage(&mut self) -> Option<ManyError> {
        self.storage
            .commit(&[])
            .map(|_| None)
            .map_err(error::storage_commit_failed)
            .unwrap_or_else(Some)
            
    }

    pub fn apply(&mut self, batch: &Batch) -> Option<Error> {
        batch
            .iter()
            .for_each(|(key, _)| self.keys.push(key.to_vec()));
        self.storage
            .apply(batch)
            .map(|_| None)
            .unwrap_or_else(Some)
    }

    pub fn commit(&mut self, batch: &Batch) -> Option<Error> {
        self.storage
            .commit(batch)
            .map(|_| None)
            .unwrap_or_else(Some)
    }

    pub fn get<'b>(&mut self, key: impl Into<Cow<'b, [u8]>>) -> KeyRetrievalResult {
        let key = key.into();
        self.keys.push(key.to_vec());
        self.storage
            .get(key.as_ref())
            .map(|result| {
                result
                    .map(KeyRetrievalResult::Retrieved)
                    .unwrap_or(KeyRetrievalResult::NotFound)
            })
            .unwrap_or_else(KeyRetrievalResult::Error)
    }

    fn prove_state(
        &self,
    ) -> Option<TrySendError<ProofResult>> {
        self.context.as_ref().prove(|| {
            self.storage
                .prove({
                    let mut query = Query::new();
                    self.keys.iter().for_each(|key| query.insert_key(key.to_vec()));
                    query
                })
                .and_then(|proof| {
                    Decoder::new(proof.as_slice())
                        .map(|fallible_operation| {
                            fallible_operation.map(|operation| match operation {
                                Child => ProofOperation::Child,
                                Parent => ProofOperation::Parent,
                                Push(Hash(hash)) => ProofOperation::NodeHash(hash.to_vec()),
                                Push(KV(key, value)) => {
                                    ProofOperation::KeyValuePair(key.into(), value.into())
                                }
                                Push(KVHash(hash)) => ProofOperation::KeyValueHash(hash.to_vec()),
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .map_err(|error| ManyError::unknown(error.to_string()))
        })
    }

    #[inline]
    fn maybe_commit(&mut self) -> Option<Error> {
        if !self.blockchain {
            self.commit(&[])
        } else {
            None
        }
        
    }
}

impl<'a> Drop for ProvingStore<'a> {
    fn drop(&mut self) {
        self.prove_state().map(|_| ()).unwrap_or_default()
    }
}
