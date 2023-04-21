use {
    crate::storage::InnerStorage,
    linkme::distributed_slice,
    many_error::ManyError,
    many_migration::{InnerMigration, MigrationSet},
    std::path::PathBuf,
};

pub mod block_9400;
pub mod data;
pub mod hash;
pub mod legacy_remove_roles;
pub mod memo;
pub mod tokens;

#[cfg(feature = "migration_testing")]
pub mod dummy_hotfix;

pub type LedgerMigrations = MigrationSet<'static, InnerStorage, PathBuf>;

// This is the global migration registry
// Doesn't contain any metadata
#[distributed_slice]
pub static MIGRATIONS: [InnerMigration<InnerStorage, ManyError, PathBuf>] = [..];
