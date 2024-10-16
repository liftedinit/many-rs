use crate::storage::InnerStorage;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::{InnerMigration, MigrationSet};

pub mod block_9400;
pub mod data;
pub mod disable_token_create;
pub mod disable_token_mint;
pub mod legacy_remove_roles;
pub mod memo;
pub mod token_create;
pub mod tokens;

#[cfg(feature = "migration_testing")]
pub mod dummy_hotfix;

pub type LedgerMigrations = MigrationSet<'static, InnerStorage>;

// This is the global migration registry
// Doesn't contain any metadata
#[distributed_slice]
pub static MIGRATIONS: [InnerMigration<InnerStorage, ManyError>] = [..];
