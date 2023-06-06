//! Tests regarding blockchain behaviour.
use many_identity::testing::identity;
use many_ledger_test_utils::*;

/// Test that out of order keys at commit in a blockchain don't cause a problem.
#[test]
fn out_of_order_keys() {
    let mut harness = Setup::new(true);
    harness.set_balance(harness.id, 1_000_000, *MFX_SYMBOL);

    let (h, a1) = harness.block(|harness| {
        harness.send_(harness.id, identity(2), 100u32);
        harness.create_account_(AccountType::Multisig)
    });
    assert_eq!(h, 1);

    let (h, a2) = harness.block(|harness| {
        let x = harness.create_account_(AccountType::Multisig);
        harness.send_(harness.id, identity(3), 100u32);
        x
    });
    assert_eq!(h, 2);
    assert_ne!(a1, a2);

    let (h, _) = harness.block(|harness| {
        harness.multisig_send_(a1, identity(4), 100u32);
        harness.multisig_send_(a2, identity(4), 100u32);
        harness.multisig_send_(a2, identity(4), 100u32);
        harness.multisig_send_(a1, identity(4), 100u32);
    });
    assert_eq!(h, 3);

    assert_eq!(harness.balance_(identity(2)), 100u32);
    assert_eq!(harness.balance_(identity(3)), 100u32);
}

/// Check that a block can query non-updated state in !Sync.
/// This DOES NOT PASS yet, but it should. Otherwise proofs won't work.
#[ignore]
#[test]
fn query_in_flight() {
    let mut harness = Setup::new(true);
    harness.set_balance(harness.id, 1_000, *MFX_SYMBOL);

    harness.block(|harness| {
        harness.send_(harness.id, identity(1), 1u32);
        harness.send_(harness.id, identity(2), 1u32);

        // This check here should still be 1_000 tokens, as we haven't committed
        // the state yet.
        assert_eq!(harness.balance_(harness.id), 1_000u32);

        harness.send_(harness.id, identity(3), 1u32);
        harness.send_(harness.id, identity(4), 1u32);
    });

    assert_eq!(harness.balance_(harness.id), 996u32);
}
