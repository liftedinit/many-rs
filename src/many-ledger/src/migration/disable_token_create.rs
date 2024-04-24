use crate::migration::MIGRATIONS;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;

#[distributed_slice(MIGRATIONS)]
pub static DISABLE_TOKEN_CREATE_MIGRATION: InnerMigration<merk::Merk, ManyError> =
    InnerMigration::new_trigger(
        false,
        "Disable Token Create Migration",
        "Disables token creation for all",
    );
