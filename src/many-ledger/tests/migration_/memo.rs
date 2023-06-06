use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::migration::memo::MEMO_MIGRATION;
use many_ledger_test_utils::*;
use many_modules::account::features::multisig;
use many_modules::events::{EventInfo, EventsModuleBackend, ListArgs};
use many_modules::{events, ledger};
use many_types::ledger::TokenAmount;
use many_types::memo::MemoLegacy;
use many_types::Memo;

#[test]
fn memo_migration_works() {
    fn make_multisig_transaction(
        h: &mut Setup,
        account_id: Address,
        legacy_memo: Option<&str>,
        legacy_data: Option<&str>,
        memo_str: Option<&str>,
        memo_data: Option<&str>,
    ) -> Vec<u8> {
        let send_tx = events::AccountMultisigTransaction::Send(ledger::SendArgs {
            from: Some(account_id),
            to: identity(10),
            symbol: *MFX_SYMBOL,
            amount: TokenAmount::from(10_000u16),
            memo: None,
        });

        let memo = match (memo_str, memo_data) {
            (Some(s), Some(d)) => {
                let mut m = Memo::try_from(s).unwrap();
                m.push_bytes(d.as_bytes().to_vec()).unwrap();
                Some(m)
            }
            (Some(s), _) => Some(Memo::try_from(s).unwrap()),
            (_, Some(d)) => Some(Memo::try_from(d.as_bytes().to_vec()).unwrap()),
            _ => None,
        };

        let tx = multisig::SubmitTransactionArgs {
            account: account_id,
            memo_: legacy_memo.map(|x| MemoLegacy::try_from(x.to_string()).unwrap()),
            transaction: Box::new(send_tx),
            threshold: None,
            timeout_in_secs: None,
            execute_automatically: None,
            data_: legacy_data.map(|x| x.as_bytes().to_vec().try_into().unwrap()),
            // This should be ignored as it would be backward incompatible
            // before the migration is active.
            memo,
        };

        multisig::AccountMultisigModuleBackend::multisig_submit_transaction(
            &mut h.module_impl,
            &identity(1),
            tx,
        )
        .map(|x| x.token.to_vec())
        .unwrap()
    }

    fn check_events<'a>(
        harness: &Setup,
        expected: impl IntoIterator<
            Item = (
                Option<&'a str>,
                Option<&'a str>,
                Option<&'a str>,
                Option<&'a str>,
            ),
        >,
    ) {
        let events = harness.module_impl.list(ListArgs::default()).unwrap();
        let mut all_events = events.events.into_iter().filter_map(|ev| {
            if let EventInfo::AccountMultisigSubmit {
                memo_, data_, memo, ..
            } = ev.content
            {
                let memo_str = memo
                    .as_ref()
                    .and_then(|x| x.iter_str().next().map(|x| x.to_owned()));
                let memo_data = memo
                    .as_ref()
                    .and_then(|x| x.iter_bytes().next().map(|x| x.to_owned()));
                Some((
                    memo_.map(|x| x.to_string()),
                    data_.map(|x| x.as_bytes().to_vec()),
                    memo_str,
                    memo_data,
                ))
            } else {
                None
            }
        });
        let mut expected = expected
            .into_iter()
            .map(|(memo_, data_, memo_str, memo_data)| {
                (
                    memo_.map(|x| x.to_string()),
                    data_.map(|x| x.as_bytes().to_vec()),
                    memo_str.map(|x| x.to_string()),
                    memo_data.map(|x| x.as_bytes().to_vec()),
                )
            });

        while let (Some(actual), Some(expected)) = (all_events.next(), expected.next()) {
            assert_eq!(actual, expected);
        }
        assert_eq!(
            all_events.next(),
            None,
            "More actual elements than expected."
        );
        assert_eq!(expected.next(), None, "More expected elements than actual.");
    }

    fn check_info(
        harness: &Setup,
        token: &[u8],
        legacy_memo: Option<&str>,
        legacy_data: Option<&str>,
        memo_str: Option<&str>,
        memo_data: Option<&str>,
    ) {
        let (legacy_data, memo_data) =
            (legacy_data.map(str::as_bytes), memo_data.map(str::as_bytes));
        let info = multisig::AccountMultisigModuleBackend::multisig_info(
            &harness.module_impl,
            &Address::anonymous(),
            multisig::InfoArgs {
                token: token.to_vec().into(),
            },
        )
        .unwrap();

        assert_eq!(info.memo_.as_ref().map(|x| x.as_ref()), legacy_memo);
        assert_eq!(info.data_.as_ref().map(|x| x.as_bytes()), legacy_data);
        assert_eq!(
            info.memo
                .as_ref()
                .and_then(|x| x.iter_str().next())
                .map(|x| x.as_str()),
            memo_str
        );
        assert_eq!(
            info.memo.as_ref().and_then(|x| x.iter_bytes().next()),
            memo_data
        );
    }

    // Setup starts with 2 accounts because of staging/ledger_state.json5
    let mut harness = Setup::new_with_migrations(true, [(8, &MEMO_MIGRATION)], false);
    harness.set_balance(harness.id, 1_000_000, *MFX_SYMBOL);
    let (_, account_id) = harness.block(|h| {
        // Create an account.
        h.create_account_as_(identity(1), AccountType::Multisig)
    });

    let (_, tx_id_1) = harness.block(|h| {
        make_multisig_transaction(
            h,
            account_id,
            Some("Legacy Memo1"),
            Some("Legacy Data1"),
            Some("Memo1"),
            None,
        )
    });
    let (_, tx_id_2) = harness.block(|h| {
        make_multisig_transaction(
            h,
            account_id,
            Some("Legacy Memo2"),
            Some("Legacy Data2"),
            None,
            None,
        )
    });
    let (_, tx_id_3) = harness.block(|h| {
        make_multisig_transaction(h, account_id, None, None, Some("Memo3"), Some("Data3"))
    });

    // Wait 4 block for the migration to run.
    // We created 4 blocks above; 1 for the account, 3 for multisig transactions.
    for _i in 0..4 {
        check_events(
            &harness,
            [
                (Some("Legacy Memo1"), Some("Legacy Data1"), None, None),
                (Some("Legacy Memo2"), Some("Legacy Data2"), None, None),
                (None, None, None, None),
            ],
        );
        check_info(
            &harness,
            &tx_id_1,
            Some("Legacy Memo1"),
            Some("Legacy Data1"),
            None,
            None,
        );
        check_info(
            &harness,
            &tx_id_2,
            Some("Legacy Memo2"),
            Some("Legacy Data2"),
            None,
            None,
        );
        check_info(&harness, &tx_id_3, None, None, None, None);

        // Migration activates in the last loop.
        harness.block(|_| {});
    }

    check_events(
        &harness,
        [
            (None, None, Some("Legacy Memo1"), Some("Legacy Data1")),
            (None, None, Some("Legacy Memo2"), Some("Legacy Data2")),
            (None, None, None, None),
        ],
    );
    check_info(
        &harness,
        &tx_id_1,
        None,
        None,
        Some("Legacy Memo1"),
        Some("Legacy Data1"),
    );
    check_info(
        &harness,
        &tx_id_2,
        None,
        None,
        Some("Legacy Memo2"),
        Some("Legacy Data2"),
    );
    check_info(&harness, &tx_id_3, None, None, None, None);

    // Add a new event after migration is active.
    let (_, new_tx_id) = harness.block(|h| {
        make_multisig_transaction(
            h,
            account_id,
            Some("Legacy4"),
            Some("Legacy4"),
            Some("Memo4"),
            Some("Data4"),
        )
    });

    check_info(
        &harness,
        &new_tx_id,
        None,
        None,
        Some("Memo4"),
        Some("Data4"),
    );
}
