use crate::migration::MIGRATIONS;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;

#[distributed_slice(MIGRATIONS)]
pub static LEGACY_REMOVE_ROLES_TRIGGER: InnerMigration<merk::Merk, ManyError> =
    InnerMigration::new_trigger(
        true,
        "LegacyRemoveRoles",
        "Trigger a legacy bug in role removal.",
    );
