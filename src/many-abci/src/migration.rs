use {
    linkme::distributed_slice,
    many_error::ManyError,
    many_migration::{InnerMigration, MigrationSet},
    std::path::PathBuf,
};

pub mod error_code;

pub type AbciAppMigrations = MigrationSet<'static, (), PathBuf>;

// This is the global migration registry
// Doesn't contain any metadata
#[distributed_slice]
pub static MIGRATIONS: [InnerMigration<(), ManyError, PathBuf>] = [..];
