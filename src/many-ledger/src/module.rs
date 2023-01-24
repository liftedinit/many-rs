use crate::error;
use crate::json::InitialStateJson;
use crate::storage::LedgerStorage;
use many_error::ManyError;
use many_migration::MigrationConfig;
use std::fmt::Debug;
use std::path::Path;
use tracing::info;

mod abci;
pub mod account;
pub mod allow_addrs;
mod data;
mod event;
mod idstore;
pub mod idstore_webauthn;
mod ledger;
mod ledger_commands;
mod ledger_mintburn;
mod ledger_tokens;
mod multisig;

/// A simple ledger that keeps transactions in memory.
#[derive(Debug)]
pub struct LedgerModuleImpl {
    storage: LedgerStorage,
}

impl LedgerModuleImpl {
    pub fn new<P: AsRef<Path>>(
        state: InitialStateJson,
        migration_config: Option<MigrationConfig>,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let symbols = state.symbols();
        let balances = state.balances()?;
        let symbols_meta = state
            .symbols_meta
            .map(|b| b.into_iter().map(|(k, v)| (k, v.into())).collect());
        let accounts = state
            .accounts
            .map(|a| a.into_iter().map(|v| v.into()).collect());

        let storage =
            LedgerStorage::new(&symbols, persistence_store_path, state.identity, blockchain)?
                .with_migrations(migration_config)?
                .with_balances(&symbols, &balances)?
                .with_idstore(state.id_store_seed, state.id_store_keys)?
                .with_tokens(
                    &symbols,
                    symbols_meta,
                    state.token_identity,
                    state.token_next_subresource,
                    balances,
                )?
                .with_account(state.account_identity, accounts)?
                .build()?;

        if let Some(h) = state.hash {
            // Verify the hash.
            let actual = hex::encode(storage.hash());
            if actual != h {
                return Err(error::invalid_initial_state(h, actual));
            }
        }

        info!(
            height = storage.get_height()?,
            hash = hex::encode(storage.hash()).as_str()
        );

        tracing::debug!("Final migrations: {:?}", storage.migrations());

        Ok(Self { storage })
    }

    pub fn load<P: AsRef<Path>>(
        migrations: Option<MigrationConfig>,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage = LedgerStorage::load(persistence_store_path, blockchain, migrations).unwrap();

        tracing::debug!("Final migrations: {:?}", storage.migrations());

        Ok(Self { storage })
    }

    #[cfg(feature = "balance_testing")]
    pub fn set_balance_only_for_testing(
        &mut self,
        account: many_identity::Address,
        balance: u64,
        symbol: many_types::ledger::Symbol,
    ) -> Result<(), ManyError> {
        self.storage
            .set_balance_only_for_testing(account, balance, symbol)?;
        Ok(())
    }
}
