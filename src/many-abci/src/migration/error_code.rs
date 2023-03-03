use crate::migration::MIGRATIONS;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;

#[distributed_slice(MIGRATIONS)]
pub static LEGACY_ERROR_CODE_TRIGGER: InnerMigration<(), ManyError> = InnerMigration::new_trigger(
    true,
    "LegacyErrorCode",
    "Trigger a legacy bug in attribute-specific error codes decoding in ABCI.",
);
