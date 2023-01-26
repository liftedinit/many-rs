use async_channel::unbounded;
use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::migration::tokens::TOKEN_MIGRATION;
use many_ledger::storage::ledger_tokens::SymbolMeta;
use many_ledger::{module::LedgerModuleImpl, storage::LedgerStorage};
use many_migration::{Metadata, MigrationConfig};
use many_modules::account::features::FeatureInfo;
use many_modules::account::AccountModuleBackend;
use many_modules::ledger::{LedgerModuleBackend, LedgerTokensModuleBackend, TokenInfoArgs};
use many_modules::{account, ledger};
use many_protocol::RequestMessage;
use std::collections::{BTreeMap, BTreeSet};

/// Verify persistent storage can be re-loaded
#[test]
fn load() {
    let path = tempfile::tempdir().unwrap().into_path();
    #[allow(unused_assignments)]
    let mut id = Address::anonymous();
    // Storage needs to become out-of-scope so it can be re-opened
    {
        let symbols = BTreeMap::from([(identity(1000), "MF0".to_string())]);
        let balances = BTreeMap::from([(
            identity(5),
            BTreeMap::from([(identity(1000), 10000000u64.into())]),
        )]);
        let _ = LedgerStorage::new(&symbols, path.clone(), identity(666), false)
            .unwrap()
            .with_balances(&symbols, &balances)
            .unwrap()
            .build()
            .unwrap();
        let mut module_impl = LedgerModuleImpl::load(None, path.clone(), false).unwrap();

        id = AccountModuleBackend::create(
            &mut module_impl,
            &identity(3),
            account::CreateArgs {
                description: None,
                roles: Some(BTreeMap::from([(
                    identity(1),
                    BTreeSet::from([account::Role::Owner]),
                )])),
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            },
        )
        .unwrap()
        .id;
    }

    let module_impl = LedgerModuleImpl::load(None, path, false).unwrap();
    let balance = module_impl
        .balance(
            &identity(5),
            ledger::BalanceArgs {
                account: Some(identity(5)),
                symbols: Some(vec![identity(1000)].into()),
            },
            (RequestMessage::default(), unbounded().0).into(),
        )
        .unwrap();
    assert_eq!(
        balance.balances,
        BTreeMap::from([(identity(1000), 10000000u64.into())])
    );

    let role = module_impl
        .get_roles(
            &identity(3),
            account::GetRolesArgs {
                account: id,
                identities: vec![identity(1)].into(),
            },
        )
        .unwrap()
        .roles;
    assert_eq!(
        role,
        BTreeMap::from([(identity(1), BTreeSet::from([account::Role::Owner]),)])
    );
}

#[test]
fn load_symbol_meta() {
    let path = tempfile::tempdir().unwrap().into_path();
    #[allow(unused_assignments)]
    let mut id = Address::anonymous();

    let migration_config = Some(MigrationConfig::default().with_migration_opts(
        &TOKEN_MIGRATION,
        Metadata {
            block_height: 0,
            disabled: false,
            issue: None,
            extra: Default::default(),
        },
    ));
    // Storage needs to become out-of-scope so it can be re-opened
    {
        let symbols = BTreeMap::from([(identity(1000), "MF0".to_string())]);
        let meta = BTreeMap::from([(
            identity(1000),
            SymbolMeta {
                name: "Foobar".to_string(),
                decimals: 9,
                owner: None,
                maximum: None,
            },
        )]);
        let initial_balance = BTreeMap::from([(
            identity(5),
            BTreeMap::from([(identity(1000), 10000000u64.into())]),
        )]);
        let _ = LedgerStorage::new(&symbols, path.clone(), identity(666), false)
            .unwrap()
            .with_migrations(migration_config.clone())
            .unwrap()
            .with_balances(&symbols, &initial_balance)
            .unwrap()
            .with_tokens(&symbols, Some(meta), None, None, initial_balance)
            .unwrap()
            .build()
            .unwrap();
        let mut module_impl =
            LedgerModuleImpl::load(migration_config.clone(), path.clone(), false).unwrap();

        id = AccountModuleBackend::create(
            &mut module_impl,
            &identity(3),
            account::CreateArgs {
                description: None,
                roles: Some(BTreeMap::from([(
                    identity(1),
                    BTreeSet::from([account::Role::Owner]),
                )])),
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            },
        )
        .unwrap()
        .id;
    }

    let module_impl = LedgerModuleImpl::load(migration_config, path, false).unwrap();
    let balance = module_impl
        .balance(
            &identity(5),
            ledger::BalanceArgs {
                account: Some(identity(5)),
                symbols: Some(vec![identity(1000)].into()),
            },
            (RequestMessage::default(), unbounded().0).into(),
        )
        .unwrap();
    assert_eq!(
        balance.balances,
        BTreeMap::from([(identity(1000), 10000000u64.into())])
    );

    let role = module_impl
        .get_roles(
            &identity(3),
            account::GetRolesArgs {
                account: id,
                identities: vec![identity(1)].into(),
            },
        )
        .unwrap()
        .roles;
    assert_eq!(
        role,
        BTreeMap::from([(identity(1), BTreeSet::from([account::Role::Owner]),)])
    );

    let info = LedgerTokensModuleBackend::info(
        &module_impl,
        &Address::anonymous(),
        TokenInfoArgs {
            symbol: identity(1000),
            extended_info: None,
        },
    )
    .unwrap()
    .info;
    assert_eq!(info.symbol, identity(1000));
    assert_eq!(info.summary.name, "Foobar".to_string());
    assert_eq!(info.summary.ticker, "MF0".to_string());
    assert_eq!(info.summary.decimals, 9);
}
