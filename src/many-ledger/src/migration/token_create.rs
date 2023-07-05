use crate::migration::MIGRATIONS;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;

#[distributed_slice(MIGRATIONS)]
pub static TOKEN_CREATE_MIGRATION: InnerMigration<merk::Merk, ManyError> =
    InnerMigration::new_trigger(
        false,
        "Token Create Migration",
        "Enables token creation for all",
    );
