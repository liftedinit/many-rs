use {
    crate::migration::MIGRATIONS, linkme::distributed_slice, many_error::ManyError,
    many_migration::InnerMigration, std::path::PathBuf,
};

#[distributed_slice(MIGRATIONS)]
pub static LEGACY_ERROR_CODE_TRIGGER: InnerMigration<(), ManyError, PathBuf> =
    InnerMigration::new_trigger(
        true,
        "LegacyErrorCode",
        "Trigger a legacy bug in attribute-specific error codes decoding in ABCI.",
    );
