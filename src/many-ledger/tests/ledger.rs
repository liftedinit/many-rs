use many_identity::Address;
use many_ledger_test_utils::*;
use many_modules::ledger;
use many_modules::ledger::{LedgerCommandsModuleBackend, LedgerModuleBackend, SendArgs};
use many_types::ledger::TokenAmount;
use proptest::prelude::*;

#[test]
fn info() {
    let Setup {
        module_impl, id, ..
    } = setup();
    let result = module_impl.info(&id, ledger::InfoArgs {});
    assert!(result.is_ok());
}

proptest! {
    #[test]
    fn balance(amount in any::<u64>()) {
        let Setup {
            mut module_impl,
            id,
            ..
        } = setup();
        module_impl.set_balance_only_for_testing(id, amount, *MFX_SYMBOL).expect("Unable to set balance for testing");
        verify_balance(&module_impl, id, *MFX_SYMBOL, amount.into());
    }
}

#[test]
fn illegal_address() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();

    module_impl
        .set_balance_only_for_testing(id, 10_000, *MFX_SYMBOL)
        .expect("Unable to set balance for testing");
    module_impl
        .send(
            &id,
            SendArgs {
                from: None,
                to: Address::illegal(),
                amount: TokenAmount::from(1_000u32),
                symbol: *MFX_SYMBOL,
                memo: None,
            },
        )
        .unwrap();

    verify_balance(&module_impl, id, *MFX_SYMBOL, 9_000u32.into());
    verify_balance(
        &module_impl,
        Address::illegal(),
        *MFX_SYMBOL,
        1_000u32.into(),
    );

    // Cannot send back from illegal.
    assert!(module_impl
        .send(
            &Address::illegal(),
            SendArgs {
                from: None,
                to: id,
                amount: TokenAmount::from(100u32),
                symbol: *MFX_SYMBOL,
                memo: None,
            },
        )
        .is_err());
    // Balances shouldn't change.
    verify_balance(&module_impl, id, *MFX_SYMBOL, 9_000u32.into());
    verify_balance(
        &module_impl,
        Address::illegal(),
        *MFX_SYMBOL,
        1_000u32.into(),
    );
}
