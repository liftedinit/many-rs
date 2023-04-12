use {
    crate::{migration::MIGRATIONS, storage::InnerStorage},
    linkme::distributed_slice,
    many_error::ManyError,
    many_migration::InnerMigration,
    std::path::PathBuf,
};

#[distributed_slice(MIGRATIONS)]
pub static LEGACY_REMOVE_ROLES_TRIGGER: InnerMigration<InnerStorage, ManyError, PathBuf> =
    InnerMigration::new_trigger(
        true,
        "LegacyRemoveRoles",
        "Trigger a legacy bug in role removal.",
    );
