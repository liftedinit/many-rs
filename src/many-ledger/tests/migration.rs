pub mod migration_;

use many_identity::testing::identity;
use many_ledger::migration::data::{
    ACCOUNT_COUNT_DATA_ATTRIBUTE, ACCOUNT_TOTAL_COUNT_INDEX, NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX,
};
use many_ledger_test_utils::*;
use many_modules::{
    data::{DataGetInfoArgs, DataModuleBackend, DataQueryArgs},
    EmptyArg,
};
use many_types::{ledger::TokenAmount, VecOrSingle};
use num_bigint::BigInt;

fn assert_metrics(harness: &Setup, expected_total: u32, expected_non_zero: u32) {
    assert_eq!(
        harness
            .module_impl
            .info(&harness.id, EmptyArg)
            .unwrap()
            .indices
            .len(),
        2
    );
    assert_eq!(
        harness
            .module_impl
            .get_info(
                &harness.id,
                DataGetInfoArgs {
                    indices: VecOrSingle(vec![
                        ACCOUNT_TOTAL_COUNT_INDEX,
                        NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX
                    ])
                }
            )
            .unwrap()
            .len(),
        2
    );
    let query = harness
        .module_impl
        .query(
            &harness.id,
            DataQueryArgs {
                indices: VecOrSingle(vec![
                    ACCOUNT_TOTAL_COUNT_INDEX,
                    NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX,
                ]),
            },
        )
        .unwrap();
    let total: BigInt = query[&ACCOUNT_TOTAL_COUNT_INDEX]
        .clone()
        .try_into()
        .unwrap();
    let non_zero: BigInt = query[&NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX]
        .clone()
        .try_into()
        .unwrap();

    assert_eq!(total, BigInt::from(expected_total));
    assert_eq!(non_zero, BigInt::from(expected_non_zero));
}

#[test]
fn migration() {
    // Setup starts with 2 accounts because of staging/ledger_state.json5
    let mut harness = Setup::new_with_migrations(true, [(2, &ACCOUNT_COUNT_DATA_ATTRIBUTE)], false);
    harness.set_balance(harness.id, 1_000_000, *MFX_SYMBOL);

    let (_height, a1) = harness.block(|h| {
        h.send_(h.id, identity(2), 250_000u32);
        identity(2)
    });

    let balance = harness.balance(a1, *MFX_SYMBOL).unwrap();

    assert_eq!(balance, 250_000u32);

    assert_eq!(
        harness
            .module_impl
            .info(&harness.id, EmptyArg)
            .unwrap()
            .indices
            .len(),
        0
    );

    let (_height, a2) = harness.block(|h| {
        h.send_(h.id, identity(3), 250_000u32);
        identity(3)
    });

    let balance = harness.balance(a2, *MFX_SYMBOL).unwrap();

    assert_eq!(balance, 250_000u32);
    assert_eq!(
        harness.balance(harness.id, *MFX_SYMBOL).unwrap(),
        TokenAmount::from(500_000u64),
    );
    assert_metrics(&harness, 5, 5);

    let (_height, a3) = harness.block(|h| {
        h.send_(h.id, identity(4), 500_000u32);
        identity(4)
    });

    let balance = harness.balance(a3, *MFX_SYMBOL).unwrap();

    assert_eq!(balance, 500_000u32);
    assert_metrics(&harness, 6, 5);
    assert_eq!(
        harness.balance(harness.id, *MFX_SYMBOL).unwrap(),
        TokenAmount::zero()
    );
}

#[test]
fn migration_stress() {
    let mut harness = Setup::new_with_migrations(true, [(2, &ACCOUNT_COUNT_DATA_ATTRIBUTE)], false);
    harness.set_balance(harness.id, 1_000_000, *MFX_SYMBOL);

    let _ = harness.block(|h| {
        h.send_(h.id, identity(2), 500_000u32);
        identity(2)
    });

    let _ = harness.block(|h| {
        h.send_(h.id, identity(3), 500_000u32);
        identity(3)
    });
    let balance = harness.balance(harness.id, *MFX_SYMBOL).unwrap();
    assert_eq!(balance, 0u32);
    let balance2 = harness.balance(identity(2), *MFX_SYMBOL).unwrap();
    assert_eq!(balance2, 500_000u32);
    let balance3 = harness.balance(identity(3), *MFX_SYMBOL).unwrap();
    assert_eq!(balance3, 500_000u32);
    assert_metrics(&harness, 5, 4);

    let _ = harness.block(|h| {
        h.send_(identity(2), h.id, 500_000u32);
        h.send_(identity(3), h.id, 500_000u32);
    });
    let balance = harness.balance(harness.id, *MFX_SYMBOL).unwrap();
    assert_eq!(balance, 1_000_000u32);
    let balance2 = harness.balance(identity(2), *MFX_SYMBOL).unwrap();
    assert_eq!(balance2, 0u32);
    let balance3 = harness.balance(identity(3), *MFX_SYMBOL).unwrap();
    assert_eq!(balance3, 0u32);
    assert_metrics(&harness, 5, 3);
}
