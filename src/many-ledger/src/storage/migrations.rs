use crate::migration::{LedgerMigrations, MIGRATIONS};
use crate::storage::LedgerStorage;
use many_error::ManyError;
use many_migration::{MigrationConfig, MigrationSet};

impl LedgerStorage {
    pub fn with_migrations(
        mut self,
        migration_config: Option<MigrationConfig>,
    ) -> Result<Self, ManyError> {
        // NOTE: Migrations are only applied in blockchain mode when loading an existing DB
        //       It is currently NOT possible to run new code in non-blockchain mode when loading an existing DB
        self.migrations = migration_config
            .map_or_else(MigrationSet::empty, |config| {
                LedgerMigrations::load(&MIGRATIONS, config, 0)
            })
            .map_err(ManyError::unknown)?; // TODO: Custom error

        Ok(self)
    }
}
