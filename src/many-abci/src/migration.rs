use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::{InnerMigration, MigrationSet};

pub mod error_code;

pub type AbciAppMigrations = MigrationSet<'static, ()>;

// This is the global migration registry
// Doesn't contain any metadata
#[distributed_slice]
pub static MIGRATIONS: [InnerMigration<(), ManyError>] = [..];
