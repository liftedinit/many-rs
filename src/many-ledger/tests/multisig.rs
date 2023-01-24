use many_error::ManyError;
use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::module::LedgerModuleImpl;
use many_ledger_test_utils::*;
use many_modules::account::features::multisig::AccountMultisigModuleBackend;
use many_modules::account::features::{multisig, TryCreateFeature};
use many_modules::{account, events, ledger};
use many_types::ledger::TokenAmount;
use proptest::prelude::*;
use proptest::test_runner::Config;
use std::collections::{BTreeMap, BTreeSet};

/// Returns informations about the given account
fn account_info(
    module_impl: &mut LedgerModuleImpl,
    id: &Address,
    account_id: Address,
) -> account::InfoReturn {
    account::AccountModuleBackend::info(
        module_impl,
        id,
        account::InfoArgs {
            account: account_id,
        },
    )
    .unwrap()
}

/// Returns the multisig account feature arguments
fn account_arguments(
    module_impl: &mut LedgerModuleImpl,
    id: &Address,
    account_id: Address,
) -> multisig::MultisigAccountFeatureArg {
    account_info(module_impl, id, account_id)
        .features
        .get::<multisig::MultisigAccountFeature>()
        .unwrap()
        .arg
}

/// Generate some SubmitTransactionArgs for testing
fn submit_args(
    account_id: Address,
    transaction: events::AccountMultisigTransaction,
    execute_automatically: Option<bool>,
) -> multisig::SubmitTransactionArgs {
    multisig::SubmitTransactionArgs {
        account: account_id,
        memo: None,
        transaction: Box::new(transaction),
        threshold: None,
        timeout_in_secs: None,
        execute_automatically,
        data_: None,
        memo_: None,
    }
}

/// Returns the multisig transaction info
fn tx_info(
    module_impl: &mut LedgerModuleImpl,
    id: Address,
    token: &minicbor::bytes::ByteVec,
) -> multisig::InfoReturn {
    let result = module_impl.multisig_info(
        &id,
        multisig::InfoArgs {
            token: token.clone(),
        },
    );
    assert!(result.is_ok());
    result.unwrap()
}

/// Return the transaction approbation status for the given identity
fn get_approbation(info: &multisig::InfoReturn, id: &Address) -> bool {
    if let Some(value) = info.approvers.get(id) {
        value.approved
    } else {
        panic!("Can't verify approbation; ID not found")
    }
}

#[test]
/// Veryfy owner can set new defaults
fn set_defaults() {
    let SetupWithAccount {
        mut module_impl,
        id,
        account_id,
    } = setup_with_account(AccountType::Multisig);
    let result = module_impl.multisig_set_defaults(
        &id,
        multisig::SetDefaultsArgs {
            account: account_id,
            threshold: Some(1),
            timeout_in_secs: Some(12),
            execute_automatically: Some(true),
        },
    );
    assert!(result.is_ok());

    let arguments = account_arguments(&mut module_impl, &id, account_id);
    assert_eq!(arguments.threshold, Some(1));
    assert_eq!(arguments.timeout_in_secs, Some(12));
    assert_eq!(arguments.execute_automatically, Some(true));
}

proptest! {
    #![proptest_config(Config { cases: 200, source_file: Some("tests/multisig"), .. Config::default() })]

    #[test]
    /// Verify owner can submit a transaction
    fn submit_transaction(SetupWithAccountAndTx { mut module_impl, id, account_id, tx } in setup_with_account_and_tx(AccountType::Multisig)) {
        let submit_args = submit_args(account_id, tx.clone(), None);
        let result = module_impl.multisig_submit_transaction(&id, submit_args.clone());
        assert!(result.is_ok());

        let tx_info = tx_info(&mut module_impl, id, &result.unwrap().token);
        assert_eq!(tx_info.memo, submit_args.memo);
        assert_eq!(tx_info.transaction, tx);
        assert_eq!(tx_info.submitter, id);
        assert!(get_approbation(&tx_info, &id));
        assert_eq!(tx_info.threshold, 3);
        assert!(!tx_info.execute_automatically);
        assert_eq!(tx_info.data_, submit_args.data_);
        assert_eq!(tx_info.memo_, submit_args.memo_);
    }

    #[test]
    /// Verify identity with `canMultisigSubmit` can submit a transaction
    fn submit_transaction_valid_role(SetupWithAccountAndTx { mut module_impl, account_id, tx, .. } in setup_with_account_and_tx(AccountType::Multisig)) {
        let result =
            module_impl.multisig_submit_transaction(&identity(3), submit_args(account_id, tx, None));
        assert!(result.is_ok());
    }

    #[test]
    /// Verify identity with `canMultisigApprove` can't submit a transaction
    fn submit_transaction_invalid_role(SetupWithAccountAndTx { mut module_impl, account_id, tx, .. } in setup_with_account_and_tx(AccountType::Multisig)) {
        let result =
            module_impl.multisig_submit_transaction(&identity(2), submit_args(account_id, tx, None));
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code(),
            account::errors::user_needs_role("").code()
        );
    }

    #[test]
    /// Verify non-owner are unable to change the defaults
    fn set_defaults_invalid_user(seed in 4..u32::MAX) {
        let SetupWithAccount {
            mut module_impl,
            id,
            account_id,
        } = setup_with_account(AccountType::Multisig);
        let result = module_impl.multisig_set_defaults(
            &identity(seed),
            account::features::multisig::SetDefaultsArgs {
                account: account_id,
                threshold: Some(1),
                timeout_in_secs: Some(12),
                execute_automatically: Some(true),
            },
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code(),
            account::errors::user_needs_role("").code()
        );

        let arguments = account_arguments(&mut module_impl, &id, account_id);
        assert_eq!(arguments.threshold, Some(3));
        assert_eq!(
            arguments.timeout_in_secs,
            Some(many_ledger::storage::multisig::MULTISIG_DEFAULT_TIMEOUT_IN_SECS)
        );
        assert_eq!(
            arguments.execute_automatically,
            Some(many_ledger::storage::multisig::MULTISIG_DEFAULT_EXECUTE_AUTOMATICALLY)
        );
    }

    #[test]
    /// Verify identity with `canMultisigApprove` and identity with `canMultisigSubmit` can approve a transaction
    fn approve(SetupWithAccountAndTx { mut module_impl, id, account_id, tx } in setup_with_account_and_tx(AccountType::Multisig)) {
        let result = module_impl.multisig_submit_transaction(&id, submit_args(account_id, tx, None));
        assert!(result.is_ok());
        let submit_return = result.unwrap();
        let info = tx_info(&mut module_impl, id, &submit_return.token);
        assert!(get_approbation(&info, &id));
        assert_eq!(info.threshold, 3);

        let result = module_impl.multisig_approve(
            &identity(2),
            multisig::ApproveArgs {
                token: submit_return.clone().token,
            },
        );
        assert!(result.is_ok());
        assert!(get_approbation(
            &tx_info(&mut module_impl, id, &submit_return.token),
            &identity(2)
        ));

        let result = module_impl.multisig_approve(
            &identity(3),
            multisig::ApproveArgs {
                token: submit_return.clone().token,
            },
        );
        assert!(result.is_ok());
        assert!(get_approbation(
            &tx_info(&mut module_impl, id, &submit_return.token),
            &identity(3)
        ));
    }

    #[test]
    /// Verify identity not part of the account can't approve a transaction
    fn approve_invalid(SetupWithAccountAndTx { mut module_impl, id, account_id, tx } in setup_with_account_and_tx(AccountType::Multisig)) {
        let result = module_impl.multisig_submit_transaction(&id, submit_args(account_id, tx, None));
        assert!(result.is_ok());
        let submit_return = result.unwrap();
        let info = tx_info(&mut module_impl, id, &submit_return.token);
        assert!(get_approbation(&info, &id));
        assert_eq!(info.threshold, 3);

        let result = module_impl.multisig_approve(
            &identity(6),
            multisig::ApproveArgs {
                token: submit_return.clone().token,
            },
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code(),
            multisig::errors::user_cannot_approve_transaction().code()
        );
    }

    #[test]
    /// Verify identity with `owner`, `canMultisigSubmit` and `canMultisigApprove` can revoke a transaction
    fn revoke(SetupWithAccountAndTx { mut module_impl, id, account_id, tx } in setup_with_account_and_tx(AccountType::Multisig)) {
        let result = module_impl.multisig_submit_transaction(&id, submit_args(account_id, tx, None));
        assert!(result.is_ok());
        let token = result.unwrap().token;
        let info = tx_info(&mut module_impl, id, &token);
        assert!(get_approbation(&info, &id));
        assert_eq!(info.threshold, 3);

        for i in [id, identity(2), identity(3)] {
            let result = module_impl.multisig_approve(
                &i,
                multisig::ApproveArgs {
                    token: token.clone(),
                },
            );
            assert!(result.is_ok());
            assert!(get_approbation(&tx_info(&mut module_impl, i, &token), &i));

            let result = module_impl.multisig_revoke(
                &i,
                multisig::RevokeArgs {
                    token: token.clone(),
                },
            );
            assert!(result.is_ok());
            assert!(!get_approbation(&tx_info(&mut module_impl, i, &token), &i));
        }
    }

    #[test]
    /// Verify identity not part of the account can't revoke a transaction
    fn revoke_invalid(SetupWithAccountAndTx { mut module_impl, id, account_id, tx } in setup_with_account_and_tx(AccountType::Multisig)) {
        let result = module_impl.multisig_submit_transaction(&id, submit_args(account_id, tx, None));
        assert!(result.is_ok());
        let token = result.unwrap().token;
        assert!(get_approbation(&tx_info(&mut module_impl, id, &token), &id));

        let result = module_impl.multisig_revoke(&identity(6), multisig::RevokeArgs { token });
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code(),
            multisig::errors::user_cannot_approve_transaction().code()
        );
    }

    #[test]
    /// Verify we can execute a transaction when the threshold is reached
    /// Both manual and automatic execution are tested
    fn execute(execute_automatically in any::<bool>(), SetupWithAccountAndTx { mut module_impl, id, account_id, tx } in setup_with_account_and_tx(AccountType::Multisig)) {
        module_impl.set_balance_only_for_testing(
            account_id,
            10000,
            *MFX_SYMBOL,
        ).expect("Unable to set balance for testing.");
        let result = module_impl.multisig_submit_transaction(&id, submit_args(account_id, tx, Some(execute_automatically)));
        assert!(result.is_ok());
        let token = result.unwrap().token;
        let info = tx_info(&mut module_impl, id, &token);
        assert!(get_approbation(&info, &id));
        assert_eq!(info.threshold, 3);

        let identities = [id, identity(2), identity(3)];
        let last = identities.last().unwrap();
        for i in identities.into_iter() {
            // Approve with the current identity
            let result = module_impl.multisig_approve(
                &i,
                account::features::multisig::ApproveArgs {
                    token: token.clone(),
                },
            );
            assert!(result.is_ok());

            // Try to execute the transaction. It should error for every
            // identity since the last identity is NOT an owner nor the
            // submitter of the transaction
            let result = module_impl.multisig_execute(
                &i,
                account::features::multisig::ExecuteArgs {
                    token: token.clone(),
                },
            );
            assert!(result.is_err());

            if &i == last {
                // At this point, everyone has approved. We can execute the
                // transaction using the owner/submitter identity.
                let result = module_impl.multisig_execute(
                    &id,
                    account::features::multisig::ExecuteArgs {
                        token: token.clone(),
                    },
                );
                if execute_automatically {
                    // Transaction was automatically executed, trying to execute
                    // it manually returns an error.
                    assert!(result.is_err());
                    assert_eq!(
                        result.unwrap_err().code(),
                        account::features::multisig::errors::transaction_expired_or_withdrawn().code()
                    );
                } else {
                    // We have enough approvers and the manual execution succeeded.
                    assert!(result.is_ok());
                    assert!(result.unwrap().data.is_ok());
                }
            } else {
                // Not enough approbation for execution yet.
                assert!(result.is_err());
                assert_eq!(
                    result.unwrap_err().code(),
                    account::features::multisig::errors::cannot_execute_transaction().code()
                );
                assert!(get_approbation(&tx_info(&mut module_impl, i, &token), &i));
            }
        }
    }

    #[test]
    /// Verify identities with `owner` and `canMultisigSubmit` can withdraw a transaction
    fn withdraw(SetupWithAccountAndTx { mut module_impl, id, account_id, tx } in setup_with_account_and_tx(AccountType::Multisig)) {
        for i in [id, identity(3)] {
            let result =
                module_impl.multisig_submit_transaction(&i, submit_args(account_id, tx.clone(), None));
            assert!(result.is_ok());
            let token = result.unwrap().token;

            let result = module_impl.multisig_withdraw(
                &i,
                multisig::WithdrawArgs {
                    token: token.clone(),
                },
            );
            assert!(result.is_ok());
            let result = module_impl.multisig_info(&i, multisig::InfoArgs { token }).unwrap();
            assert_eq!(result.state, multisig::MultisigTransactionState::Withdrawn);
        }
    }

    #[test]
    /// Verify identity with `canMultisigApprove` and identity not part of the account can't withdraw a transaction
    fn withdraw_invalid(SetupWithAccountAndTx { mut module_impl, id, account_id, tx } in setup_with_account_and_tx(AccountType::Multisig)) {
        let result = module_impl.multisig_submit_transaction(&id, submit_args(account_id, tx, None));
        assert!(result.is_ok());
        let token = result.unwrap().token;
        for i in [identity(2), identity(6)] {
            let result = module_impl.multisig_withdraw(
                &i,
                multisig::WithdrawArgs {
                    token: token.clone(),
                },
            );
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().code(),
                multisig::errors::cannot_execute_transaction().code()
            );
        }
    }
}

#[test]
/// Verify that transactions expire after a while.
fn expires() {
    let mut setup = Setup::new(true);
    let account_id = setup.create_account_(AccountType::Multisig);
    let owner_id = setup.id;

    let (h, token) = setup.block(|setup| setup.multisig_send_(account_id, identity(3), 10u32));
    assert_eq!(h, 1);

    let (h, ()) = setup.block(|_| {});
    assert_eq!(h, 2);

    // Assert that it still exists and is not disabled.
    setup.assert_multisig_info(&token, |i| {
        assert_eq!(
            i.state,
            multisig::MultisigTransactionState::Pending,
            "State: {i:#?}"
        );
    });

    setup.inc_time(1_000_000);
    let (h, ()) = setup.block(|_| {});
    assert_eq!(h, 3);

    setup.assert_multisig_info(&token, |i| {
        assert_eq!(i.state, multisig::MultisigTransactionState::Expired);
    });

    // Can't approve.
    setup.block(|setup| {
        assert_eq!(
            setup.multisig_approve(owner_id, &token),
            Err(multisig::errors::transaction_expired_or_withdrawn())
        );
    });
}

/// Verifies that multiple transactions can be in flight and resolved separately.
#[test]
fn multiple_multisig() {
    let mut setup = Setup::new(true);
    setup.set_balance(setup.id, 1_000_000, *MFX_SYMBOL);
    let account_ids: Vec<Address> = (0..5)
        .map(|_| setup.create_account_(AccountType::Multisig))
        .collect();

    // Create 3 transactions in a block.
    let (h, mut tokens) = setup.block(|setup| {
        // Does not validate when created.
        vec![
            setup.multisig_send_(account_ids[0], identity(3), 10u32),
            setup.multisig_send_(account_ids[1], identity(4), 15u32),
            setup.multisig_send_(account_ids[2], identity(5), 20u32),
        ]
    });
    assert_eq!(h, 1);

    // Create 3 more transactions in a block.
    let (h, mut tokens2) = setup.block(|setup| {
        vec![
            setup.multisig_send_(account_ids[0], identity(6), 10u32),
            setup.multisig_send_(account_ids[1], identity(7), 15u32),
            setup.multisig_send_(account_ids[2], identity(8), 20u32),
        ]
    });
    assert_eq!(h, 2);
    tokens.append(&mut tokens2);

    // Approve 4 of them in a block. Execute 2.
    let (h, _) = setup.block(|setup| {
        setup.multisig_approve_(identity(2), &tokens[0]);
        setup.multisig_approve_(identity(2), &tokens[1]);
        setup.multisig_approve_(identity(2), &tokens[2]);
        setup.multisig_approve_(identity(2), &tokens[3]);
        assert_eq!(
            setup.multisig_execute(&tokens[2]).unwrap_err(),
            multisig::errors::cannot_execute_transaction(),
        );

        setup.multisig_approve_(identity(3), &tokens[2]);
        setup.multisig_approve_(identity(3), &tokens[3]);

        // Is okay.
        setup.send_(setup.id, account_ids[2], 100u32);
        let data = setup.multisig_execute_(&tokens[2]).data;
        assert!(data.is_ok(), "Err: {}", data.unwrap_err());

        // Insufficient funds.
        assert_many_err(
            setup.multisig_execute_(&tokens[3]).data,
            ledger::insufficient_funds(),
        );
    });
    assert_eq!(h, 3);

    setup.assert_multisig_info(&tokens[0], |i| {
        assert_eq!(i.state, multisig::MultisigTransactionState::Pending);
    });
    setup.assert_multisig_info(&tokens[1], |i| {
        assert_eq!(i.state, multisig::MultisigTransactionState::Pending);
    });
    setup.assert_multisig_info(&tokens[2], |i| {
        assert_eq!(
            i.state,
            multisig::MultisigTransactionState::ExecutedManually
        );
    });
    setup.assert_multisig_info(&tokens[3], |i| {
        assert_eq!(
            i.state,
            multisig::MultisigTransactionState::ExecutedManually
        );
    });
    setup.assert_multisig_info(&tokens[4], |i| {
        assert_eq!(i.state, multisig::MultisigTransactionState::Pending);
    });

    assert_eq!(setup.balance_(account_ids[0]), 0u16);
    assert_eq!(setup.balance_(account_ids[1]), 0u16);
    assert_eq!(setup.balance_(account_ids[2]), 80u16);
    assert_eq!(setup.balance_(identity(5)), 20u16);
    assert_eq!(setup.balance_(account_ids[3]), 0u16);
    assert_eq!(setup.balance_(account_ids[4]), 0u16);
}

#[test]
// Send funds on behalf on another account using a multisig
// Both the sender and the account from which the funds are transfered only have the Multisig feature
fn multisig_send_from_another_identity_owner() {
    let mut setup = Setup::new(false);

    // Create two accounts with different owners
    let acc1 = setup.create_account_as_(setup.id, AccountType::Multisig);
    let acc2 = setup.create_account_as_(identity(666), AccountType::Multisig);

    setup.set_balance(acc2, 1_000_000, *MFX_SYMBOL);

    // Prepare a Send transaction from acc2 to some Address
    let send_tx = events::AccountMultisigTransaction::Send(ledger::SendArgs {
        from: Some(acc2),
        to: identity(1234),
        symbol: *MFX_SYMBOL,
        amount: TokenAmount::from(10u16),
        memo: None,
    });

    // Create a multisig tx on acc1 which sends funds from acc2 to some Address
    let tx = setup.create_multisig_as(acc1, acc1, send_tx.clone());
    let token = tx.unwrap();

    // Approve the tx
    setup.multisig_approve_(identity(2), &token);
    setup.multisig_approve_(identity(3), &token);

    // Execute the tx
    let response = setup.multisig_execute_(&token);

    // Execution fails because acc1 is don't have the required permission on acc2
    assert!(response.data.is_err());
    assert_many_err(
        response.data,
        account::errors::user_needs_role("canLedgerTransact"),
    );

    // Let's add acc1 as an owner of acc2 using identity(666) as the sender, which is the owner of acc2
    setup.add_roles_as(
        identity(666),
        acc2,
        BTreeMap::from([(acc1, BTreeSet::from([account::Role::Owner]))]),
    );

    // Recreate the tx and approve it
    let tx = setup.create_multisig_as(acc1, acc1, send_tx);
    let token = tx.unwrap();
    setup.multisig_approve_(identity(2), &token);
    setup.multisig_approve_(identity(3), &token);

    // At this point, acc1 is an owner of acc2. Multisig tx execution should work
    let response = setup.multisig_execute_(&token);
    assert!(response.data.is_ok());
    assert_eq!(setup.balance_(identity(1234)), 10u16);
    assert_eq!(setup.balance_(acc2), 999_990u32);
}

#[test]
// Send funds on behalf on another account using a multisig
// The sender account only has the Multisig feature
// The account from which the funds are transfered only have the Ledger feature
fn multisig_send_from_another_identity_with_perm() {
    let mut setup = Setup::new(false);

    // Create two accounts with different owners
    let acc1 = setup.create_account_as_(setup.id, AccountType::Multisig);
    let acc2 = setup.create_account_as_(identity(666), AccountType::Ledger);

    setup.set_balance(acc2, 1_000_000, *MFX_SYMBOL);

    // Prepare a Send transaction from acc2 to some Address.
    // acc2 doesn't have the Multisig feature
    let send_tx = events::AccountMultisigTransaction::Send(ledger::SendArgs {
        from: Some(acc2),
        to: identity(1234),
        symbol: *MFX_SYMBOL,
        amount: TokenAmount::from(10u16),
        memo: None,
    });

    // Create a multisig tx on acc1 which sends funds from acc2 to some Address
    let tx = setup.create_multisig_as(acc1, acc1, send_tx.clone());
    let token = tx.unwrap();

    // Approve the tx
    setup.multisig_approve_(identity(2), &token);
    setup.multisig_approve_(identity(3), &token);

    // Execute the tx
    let response = setup.multisig_execute_(&token);

    // Execution fails because acc1 is NOT owner on acc2
    assert!(response.data.is_err());
    assert_many_err(
        response.data,
        account::errors::user_needs_role("canLedgerTransact"),
    );

    // Let's add `canLedgerTransact` permission to acc1 on acc2 using identity(666) as the sender, which is the owner of acc2
    setup.add_roles_as(
        identity(666),
        acc2,
        BTreeMap::from([(acc1, BTreeSet::from([account::Role::CanLedgerTransact]))]),
    );

    // Recreate the tx and approve it
    let tx = setup.create_multisig_as(acc1, acc1, send_tx);
    let token = tx.unwrap();
    setup.multisig_approve_(identity(2), &token);
    setup.multisig_approve_(identity(3), &token);

    // At this point, acc1 has the rights to send funds from acc2. Multisig tx execution should work
    let response = setup.multisig_execute_(&token);
    assert!(response.data.is_ok());
    assert_eq!(setup.balance_(identity(1234)), 10u16);
    assert_eq!(setup.balance_(acc2), 999_990u32);
}

#[test]
fn recursive_multisig() {
    let mut setup = Setup::new(false);

    let acc1 = setup.create_account_as_(setup.id, AccountType::Multisig);
    let acc2 = setup.create_account_as_(identity(666), AccountType::Ledger);

    setup.set_balance(acc2, 1_000_000, *MFX_SYMBOL);

    // acc2 doesn't have the Multisig feature
    let send_tx = events::AccountMultisigTransaction::Send(ledger::SendArgs {
        from: Some(acc2),
        to: identity(1234),
        symbol: *MFX_SYMBOL,
        amount: TokenAmount::from(10u16),
        memo: None,
    });

    let multisig_tx = events::AccountMultisigTransaction::AccountMultisigSubmit(
        multisig::SubmitTransactionArgs {
            account: acc2,
            memo: None,
            transaction: Box::new(send_tx),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: Some(false),
            data_: None,
            memo_: None,
        },
    );

    // Create a multisig on acc1 which contains a multisig submit on acc2 which sends funds from acc2 to some Address
    let tx = setup.create_multisig_as(acc1, acc1, multisig_tx.clone());
    let token = tx.unwrap();
    setup.multisig_approve_(identity(2), &token);
    setup.multisig_approve_(identity(3), &token);

    // The execution should fail because acc1 do NOT have the permission to submit a multisig on behalf of acc2
    let response = setup.multisig_execute_(&token);
    assert!(response.data.is_err());
    assert_many_err(
        response.data,
        account::errors::user_needs_role("canMultisigSubmit"),
    );

    // Let's add `canMultisigSubmit` permission to acc1 on acc2 using identity(666) as the sender, which is the owner of acc2
    setup.add_roles_as(
        identity(666),
        acc2,
        BTreeMap::from([(acc1, BTreeSet::from([account::Role::CanMultisigSubmit]))]),
    );

    // Re-create the multisig and re-execute it
    let tx = setup.create_multisig_as(acc1, acc1, multisig_tx);
    let token = tx.unwrap();
    setup.multisig_approve_(identity(2), &token);
    setup.multisig_approve_(identity(3), &token);

    // This time, the execution should fail because acc2 doesn't have the Multisig account feature
    let response = setup.multisig_execute_(&token);
    assert!(response.data.is_err());
    assert_many_err(
        response.data,
        ManyError::attribute_not_found(multisig::MultisigAccountFeature::ID),
    );

    // Let's make acc2 a Multisig account
    let acc2 = setup.create_account_as_(identity(666), AccountType::Multisig);
    setup.set_balance(acc2, 1_000_000, *MFX_SYMBOL);

    // Recreate the tx
    let send_tx = events::AccountMultisigTransaction::Send(ledger::SendArgs {
        from: Some(acc2),
        to: identity(1234),
        symbol: *MFX_SYMBOL,
        amount: TokenAmount::from(10u16),
        memo: None,
    });

    let multisig_tx = events::AccountMultisigTransaction::AccountMultisigSubmit(
        multisig::SubmitTransactionArgs {
            account: acc2,
            memo: None,
            transaction: Box::new(send_tx),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
            data_: None,
            memo_: None,
        },
    );

    // Let's add `canMultisigSubmit` permission to acc1 on acc2 using identity(666) as the sender, which is the owner of acc2
    setup.add_roles_as(
        identity(666),
        acc2,
        BTreeMap::from([(acc1, BTreeSet::from([account::Role::CanMultisigSubmit]))]),
    );

    let tx = setup.create_multisig_as(acc1, acc1, multisig_tx);
    let token = tx.unwrap();
    setup.multisig_approve_(identity(2), &token);
    setup.multisig_approve_(identity(3), &token);

    // Execute the tx. Sender is setup.id which is an owner of acc1
    let response = setup.multisig_execute_(&token);
    assert!(response.data.is_ok());

    // At this point we submitted a new Multisig tx to send funds from acc2 to some Address
    let result: account::features::multisig::SubmitTransactionReturn =
        minicbor::decode(&response.data.unwrap()).unwrap();
    let token = result.token;

    // Approve and execute the tx
    setup.multisig_approve_(identity(2), &token);
    setup.multisig_approve_(identity(3), &token);

    // Execute the tx as acc2 which owns itself
    let response = setup.multisig_execute_as_(acc2, &token);
    assert!(response.data.is_ok());
    assert_eq!(setup.balance_(identity(1234)), 10u16);
    assert_eq!(setup.balance_(acc2), 999_990u32);
}

#[test]
// Issue #179
fn approve_executed_tx() {
    let mut setup = Setup::new(false);
    let acc1 = setup.create_account(AccountType::Multisig).unwrap();
    setup.set_balance(acc1, 1_000_000, *MFX_SYMBOL);

    let token = setup.multisig_send_(acc1, identity(1234), 10u16);
    setup.multisig_approve_(setup.id, &token);
    setup.multisig_approve_(identity(2), &token);
    setup.multisig_approve_(identity(3), &token);

    let response = setup.multisig_execute_(&token);
    assert!(response.data.is_ok());
    assert_eq!(setup.balance_(acc1), 999_990u32);
    assert_eq!(setup.balance_(identity(1234)), 10u16);

    setup.add_roles(
        acc1,
        BTreeMap::from([(
            identity(6),
            BTreeSet::from([account::Role::CanMultisigSubmit]),
        )]),
    );
    let result = setup.multisig_approve(identity(6), &token);
    assert_many_err(result, multisig::errors::transaction_expired_or_withdrawn());
}
