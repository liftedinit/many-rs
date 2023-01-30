use many_identity::testing::identity;
use many_ledger::error;
use many_ledger_test_utils::*;
use many_modules::ledger;
use many_modules::ledger::LedgerCommandsModuleBackend;
use proptest::prelude::*;

proptest! {
    #[test]
    fn send(amount in any::<u64>()) {
        let Setup {
            mut module_impl,
            id,
            ..
        } = setup();
        let half = amount / 2;
        module_impl.set_balance_only_for_testing(id, amount, *MFX_SYMBOL).expect("Unable to set balance for testing.");
        let result = module_impl.send(&id, ledger::SendArgs {
            from: Some(id),
            to: identity(1),
            amount: half.into(),
            symbol: *MFX_SYMBOL,
            memo: None,
        });
        assert!(result.is_ok());
        verify_balance(&module_impl, id, *MFX_SYMBOL, (amount - half).into());
        verify_balance(&module_impl, identity(1), *MFX_SYMBOL, half.into());
    }

    #[test]
    fn send_account(amount in any::<u64>()) {
        let SetupWithAccount {
            mut module_impl,
            account_id,
            id,
        } = setup_with_account(AccountType::Ledger);
        let half = amount / 2;
        module_impl.set_balance_only_for_testing(account_id, amount, *MFX_SYMBOL).expect("Unable to set balance for testing.");
        let result = module_impl.send(&id, ledger::SendArgs {
            from: Some(account_id),
            to: identity(1),
            amount: half.into(),
            symbol: *MFX_SYMBOL,
            memo: None,
        });
        assert!(result.is_ok());
        verify_balance(&module_impl, account_id, *MFX_SYMBOL, (amount - half).into());
        verify_balance(&module_impl, identity(1), *MFX_SYMBOL, half.into());
    }
}

#[test]
fn send_account_missing_feature() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        ..
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.send(
        &identity(2),
        ledger::SendArgs {
            from: Some(account_id),
            to: identity(1),
            amount: 10u16.into(),
            symbol: *MFX_SYMBOL,
            memo: None,
        },
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), error::unauthorized().code());
}

#[test]
fn send_invalid_account() {
    let SetupWithAccount {
        mut module_impl,
        id,
        ..
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.send(
        &id,
        ledger::SendArgs {
            from: Some(identity(6)),
            to: identity(1),
            amount: 10u16.into(),
            symbol: *MFX_SYMBOL,
            memo: None,
        },
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), error::unauthorized().code());
}
