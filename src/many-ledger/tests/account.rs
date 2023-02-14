use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::module::LedgerModuleImpl;
use many_ledger_test_utils::*;
use many_modules::account;
use many_modules::account::features::{FeatureInfo, TryCreateFeature};
use many_modules::account::AccountModuleBackend;
use many_types::{Either, VecOrSingle};
use std::collections::{BTreeMap, BTreeSet};

fn account_info(
    module_impl: &LedgerModuleImpl,
    id: &Address,
    account_id: &Address,
) -> account::InfoReturn {
    let result = account::AccountModuleBackend::info(
        module_impl,
        id,
        account::InfoArgs {
            account: *account_id,
        },
    );
    assert!(result.is_ok());
    result.unwrap()
}

#[test]
/// Verify we can create an account
fn create() {
    let SetupWithArgs {
        mut module_impl,
        id,
        args,
    } = setup_with_args(AccountType::Multisig);
    let result = module_impl.create(&id, args);
    assert!(result.is_ok());

    // Verify the account owns itself
    let account_id = result.unwrap().id;
    let info = account_info(&module_impl, &id, &account_id);
    assert!(info.roles.contains_key(&account_id));
    assert!(info
        .roles
        .get(&account_id)
        .unwrap()
        .contains(&account::Role::Owner));
}

#[test]
/// Verify we can't create an account with roles unsupported by feature
fn create_invalid_role() {
    let SetupWithArgs {
        mut module_impl,
        id,
        mut args,
    } = setup_with_args(AccountType::Multisig);
    if let Some(roles) = args.roles.as_mut() {
        roles.insert(
            identity(4),
            BTreeSet::from_iter([account::Role::CanLedgerTransact]),
        );
    }
    let result = module_impl.create(&id, args);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        account::errors::unknown_role("").code(),
    );
}

#[test]
/// Verify we can change the account description
fn set_description() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.set_description(
        &id,
        account::SetDescriptionArgs {
            account: account_id,
            description: "New".to_string(),
        },
    );
    assert!(result.is_ok());
    assert_eq!(
        account_info(&module_impl, &id, &account_id).description,
        Some("New".to_string())
    );
}

#[test]
/// Verify non-owner is not able to change the description
fn set_description_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.set_description(
        &identity(1),
        account::SetDescriptionArgs {
            account: account_id,
            description: "Other".to_string(),
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        account::errors::user_needs_role("owner").code()
    );
}

#[test]
/// Verify we can list account roles
fn list_roles() {
    let SetupWithAccount {
        module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.list_roles(
        &id,
        account::ListRolesArgs {
            account: account_id,
        },
    );
    assert!(result.is_ok());
    let mut roles = BTreeSet::<account::Role>::new();
    for (_, r) in account_info(&module_impl, &id, &account_id)
        .roles
        .iter_mut()
    {
        roles.append(r)
    }
    roles.remove(&account::Role::Owner);
    assert_eq!(result.unwrap().roles, roles);
}

#[test]
/// Verify we can get given identities account roles
fn get_roles() {
    let SetupWithAccount {
        module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);
    let identities = vec![identity(2), identity(3)];
    let result = module_impl.get_roles(
        &id,
        account::GetRolesArgs {
            account: account_id,
            identities: VecOrSingle::from(identities.clone()),
        },
    );
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().roles,
        account_info(&module_impl, &id, &account_id)
            .roles
            .into_iter()
            .filter(|&(k, _)| identities.contains(&k))
            .collect()
    );
}

#[test]
/// Verify we can add new roles
fn add_roles() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);
    let new_role = (
        identity(4),
        BTreeSet::from_iter([account::Role::CanLedgerTransact]),
    );
    let result = module_impl.add_roles(
        &id,
        account::AddRolesArgs {
            account: account_id,
            roles: BTreeMap::from_iter([new_role.clone()]),
        },
    );
    assert!(result.is_ok());
    let identities = vec![identity(4)];
    assert!(account_info(&module_impl, &id, &account_id)
        .roles
        .into_iter()
        .find(|&(k, _)| identities.contains(&k))
        .filter(|role| role == &new_role)
        .is_some())
}

#[test]
/// Verify non-owner is not able to add role
fn add_roles_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);
    let mut new_role = BTreeMap::from_iter([(
        identity(4),
        BTreeSet::from_iter([account::Role::CanLedgerTransact]),
    )]);
    let mut roles = account_info(&module_impl, &id, &account_id).roles;
    roles.append(&mut new_role);
    let result = module_impl.add_roles(
        &identity(2),
        account::AddRolesArgs {
            account: account_id,
            roles: new_role.clone(),
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        account::errors::user_needs_role("owner").code()
    );
}

#[test]
/// Verify we can remove roles
fn remove_roles() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.remove_roles(
        &id,
        account::RemoveRolesArgs {
            account: account_id,
            roles: BTreeMap::from_iter([(
                identity(2),
                BTreeSet::from_iter([account::Role::CanMultisigApprove]),
            )]),
        },
    );
    assert!(result.is_ok());

    let result = module_impl.get_roles(
        &id,
        account::GetRolesArgs {
            account: account_id,
            identities: VecOrSingle::from(vec![identity(2)]),
        },
    );
    assert!(result.is_ok());
    assert_eq!(
        result.unwrap().roles.get(&identity(2)).unwrap(),
        &BTreeSet::<account::Role>::new()
    );
}

#[test]
// Verify non-owner is not able to remove role
fn remove_roles_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.remove_roles(
        &identity(2),
        account::RemoveRolesArgs {
            account: account_id,
            roles: BTreeMap::from_iter([(
                identity(2),
                BTreeSet::from_iter([account::Role::CanMultisigApprove]),
            )]),
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        account::errors::user_needs_role("owner").code()
    );
}

#[test]
fn remove_owner_role() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        id,
    } = setup_with_account(AccountType::Multisig);

    // Removing the owner role from the account itself should result in an error
    let result = module_impl.remove_roles(
        &id,
        account::RemoveRolesArgs {
            account: account_id,
            roles: BTreeMap::from_iter([(account_id, BTreeSet::from_iter([account::Role::Owner]))]),
        },
    );
    assert!(result.is_err());
    assert_many_err(result, account::errors::account_must_own_itself());
}

#[test]
/// Verify we can disable account
fn disable() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.disable(
        &id,
        account::DisableArgs {
            account: account_id,
        },
    );
    assert!(result.is_ok());

    let result = AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account_id,
        },
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().disabled.unwrap(), Either::Left(true));
}

#[test]
/// Verify non-owner is unable to disable account
fn disable_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.disable(
        &identity(2),
        account::DisableArgs {
            account: account_id,
        },
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        account::errors::user_needs_role("owner").code()
    );
}

/// Verify that add_feature works with a valid feature.
#[test]
fn add_feature() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);

    let info_before = account::AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account_id,
        },
    )
    .expect("Could not get info");

    // Prevent test from regressing.
    assert!(!info_before
        .features
        .has_id(account::features::ledger::AccountLedger::ID));

    module_impl
        .add_features(
            &id,
            account::AddFeaturesArgs {
                account: account_id,
                roles: None,
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            },
        )
        .expect("Could not add feature");

    let info_after = account::AccountModuleBackend::info(
        &module_impl,
        &id,
        account::InfoArgs {
            account: account_id,
        },
    )
    .expect("Could not get info");

    assert!(info_after
        .features
        .has_id(account::features::ledger::AccountLedger::ID));
}

/// Verify that add_feature works with a valid feature.
#[test]
fn add_feature_non_owner() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);

    assert!(module_impl
        .add_features(
            &identity(4),
            account::AddFeaturesArgs {
                account: account_id,
                roles: None,
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            },
        )
        .is_err());

    let info_after = account_info(&module_impl, &id, &account_id);

    assert!(!info_after
        .features
        .has_id(account::features::ledger::AccountLedger::ID));
}

/// Verify that add_feature works with a valid feature.
#[test]
fn add_feature_and_role() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);

    let info_before = account_info(&module_impl, &id, &account_id);
    // Prevent test from regressing.
    assert!(!info_before
        .features
        .has_id(account::features::ledger::AccountLedger::ID));
    assert!(!info_before.roles.contains_key(&identity(4)));

    module_impl
        .add_features(
            &id,
            account::AddFeaturesArgs {
                account: account_id,
                roles: Some(BTreeMap::from_iter([(
                    identity(4),
                    BTreeSet::from_iter([account::Role::Owner]),
                )])),
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            },
        )
        .expect("Could not add feature");

    let info_after = account_info(&module_impl, &id, &account_id);

    assert!(info_after
        .features
        .has_id(account::features::ledger::AccountLedger::ID));
    assert!(info_after
        .roles
        .get(&identity(4))
        .unwrap()
        .contains(&account::Role::Owner));
}

/// Verify that add_feature cannot add existing features.
#[test]
fn add_feature_existing() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);

    let info_before = account_info(&module_impl, &id, &account_id);

    assert!(info_before
        .features
        .has_id(account::features::multisig::MultisigAccountFeature::ID));

    let result = module_impl.add_features(
        &id,
        account::AddFeaturesArgs {
            account: account_id,
            roles: None,
            features: account::features::FeatureSet::from_iter([
                account::features::multisig::MultisigAccountFeature::default().as_feature(),
            ]),
        },
    );
    assert!(result.is_err());

    let info_after = account_info(&module_impl, &id, &account_id);

    assert!(info_after
        .features
        .has_id(account::features::multisig::MultisigAccountFeature::ID));
}

#[test]
/// Issue #113
fn send_tx_on_behalf_as_owner() {
    let mut setup = Setup::new(false);

    // Account doesn't have feature 0
    let account_id = setup.create_account_(AccountType::Multisig);
    setup.set_balance(account_id, 1_000_000, *MFX_SYMBOL);

    // Sending as the account is permitted
    let result = setup.send_as(account_id, account_id, identity(4), 10u32, *MFX_SYMBOL);
    assert!(result.is_ok());

    // Sending as the account owner is permitted, even without feature 0
    let result = setup.send_as(setup.id, account_id, identity(4), 10u32, *MFX_SYMBOL);
    assert!(result.is_ok());

    // identity(2) should not be allowed to send on behalf of the account.
    // identity(2) is not an account owner
    let result = setup.send_as(identity(2), account_id, identity(4), 10u32, *MFX_SYMBOL);
    assert_many_err(result, many_ledger::error::unauthorized());
}

#[test]
/// Issue #169 - account.create
fn empty_feature_create() {
    let SetupWithArgs {
        mut module_impl,
        id,
        mut args,
    } = setup_with_args(AccountType::Multisig);

    // No role, no feature.
    args.roles = None;
    args.features = account::features::FeatureSet::empty();

    let result = module_impl.create(&id, args);
    assert!(result.is_err());
    assert_many_err(result, account::errors::empty_feature());
}

#[test]
/// Issue #169 - account.addFeatures
fn empty_feature_add_features() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);

    let result = module_impl.add_features(
        &id,
        account::AddFeaturesArgs {
            account: account_id,
            roles: None,
            features: account::features::FeatureSet::empty(),
        },
    );
    assert!(result.is_err());
    assert_many_err(result, account::errors::empty_feature());
}
