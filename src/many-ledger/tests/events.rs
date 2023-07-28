use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::module::LedgerModuleImpl;
use many_ledger_test_utils::*;
use many_modules::account::features::multisig::{
    self, AccountMultisigModuleBackend, MultisigTransactionState,
};
use many_modules::events::{
    self, EventFilterAttributeSpecific, EventFilterAttributeSpecificIndex, EventId,
    EventsModuleBackend,
};
use many_modules::ledger;
use many_modules::ledger::LedgerCommandsModuleBackend;
use many_types::{CborRange, Memo, Timestamp};
use proptest::prelude::*;
use proptest::test_runner::Config;
use std::collections::BTreeMap;
use std::ops::Bound;

fn send(module_impl: &mut LedgerModuleImpl, from: Address, to: Address) {
    module_impl
        .set_balance_only_for_testing(from, 1000, *MFX_SYMBOL)
        .expect("Unable to set balance for testing.");
    send_(module_impl, from, to);
}

fn send_(module_impl: &mut LedgerModuleImpl, from: Address, to: Address) {
    let result = module_impl.send(
        &from,
        ledger::SendArgs {
            from: Some(from),
            to,
            amount: 10u16.into(),
            symbol: *MFX_SYMBOL,
            memo: None,
        },
    );
    assert!(result.is_ok());
}

#[test]
fn events() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    let result = events::EventsModuleBackend::info(&module_impl, events::InfoArgs {});
    assert!(result.is_ok());
    assert_eq!(result.unwrap().total, 0);
    send(&mut module_impl, id, identity(1));
    let result = events::EventsModuleBackend::info(&module_impl, events::InfoArgs {});
    assert!(result.is_ok());
    assert_eq!(result.unwrap().total, 1);
}

#[test]
fn list() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    send(&mut module_impl, id, identity(1));
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: None,
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
}

#[test]
fn list_many() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();

    send(&mut module_impl, id, identity(1));
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: None,
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);

    send(&mut module_impl, id, identity(1));
    let list_return = module_impl
        .list(events::ListArgs {
            count: None,
            order: None,
            filter: None,
        })
        .unwrap();
    assert_eq!(list_return.nb_events, 2);
    assert_eq!(list_return.events.len(), 2);

    send(&mut module_impl, identity(1), identity(2));
    let list_return = module_impl
        .list(events::ListArgs {
            count: None,
            order: None,
            filter: None,
        })
        .unwrap();
    assert_eq!(list_return.nb_events, 3);
    assert_eq!(list_return.events.len(), 3);

    let list_return = module_impl
        .list(events::ListArgs {
            count: Some(2),
            order: None,
            filter: None,
        })
        .unwrap();
    assert_eq!(list_return.nb_events, 3);
    assert_eq!(list_return.events.len(), 2);
}

#[test]
fn list_blockchain() {
    let mut setup = Setup::new(true);
    let id = setup.id;
    setup.set_balance(id, 1000, *MFX_SYMBOL);
    setup.block(|_| {});

    let result = setup.module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: None,
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 0);
    assert_eq!(list_return.events.len(), 0);

    for i in 1..=3 {
        setup.block(|setup| {
            send_(&mut setup.module_impl, id, identity(1));
        });
        let list_return = setup
            .module_impl
            .list(events::ListArgs {
                count: None,
                order: None,
                filter: None,
            })
            .unwrap();
        assert_eq!(list_return.nb_events, i);
        assert_eq!(list_return.events.len(), i as usize);
    }

    let list_return = setup
        .module_impl
        .list(events::ListArgs {
            count: Some(2),
            order: None,
            filter: None,
        })
        .unwrap();
    assert_eq!(list_return.nb_events, 3);
    assert_eq!(list_return.events.len(), 2);
}

#[test]
fn list_filter_account() {
    let SetupWithAccount {
        mut module_impl,
        account_id,
        id,
    } = setup_with_account(AccountType::Ledger);
    send(&mut module_impl, id, identity(3));
    send(&mut module_impl, account_id, identity(1));
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            account: Some(vec![account_id].into()),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 3);
    assert_eq!(list_return.events.len(), 2); // 1 send + 1 create
    for event in list_return.events {
        match event.content {
            events::EventInfo::AccountCreate { account, .. } => {
                assert_eq!(account, account_id);
            }
            events::EventInfo::Send { from, .. } => {
                assert_eq!(from, account_id);
            }
            _ => unimplemented!(),
        }
    }
}

#[test]
fn list_filter_kind() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    send(&mut module_impl, id, identity(1));
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            kind: Some(vec![events::EventKind::Send].into()),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
    assert_eq!(list_return.events[0].kind(), events::EventKind::Send);
    assert!(list_return.events[0].is_about(*MFX_SYMBOL));
    assert!(list_return.events[0].is_about(id));
}

#[test]
fn list_filter_id() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();

    // Create 5 events
    for i in 0..5 {
        send(&mut module_impl, id, identity(i));
    }

    // Collect all events ids
    let events_ids = get_all_events_ids(&mut module_impl);

    let tests = vec![
        // Unbounded bounds
        (Bound::Unbounded, Bound::Unbounded, 5, 0..5),
        // Included bounds
        (
            Bound::Unbounded,
            Bound::Included(events_ids[3].clone()),
            4,
            0..4,
        ),
        (
            Bound::Included(events_ids[1].clone()),
            Bound::Unbounded,
            4,
            1..5,
        ),
        (
            Bound::Included(events_ids[1].clone()),
            Bound::Included(events_ids[3].clone()),
            3,
            1..4,
        ),
        // Excluded bounds
        (
            Bound::Unbounded,
            Bound::Excluded(events_ids[4].clone()),
            4,
            0..4,
        ),
        (
            Bound::Excluded(events_ids[0].clone()),
            Bound::Unbounded,
            4,
            1..5,
        ),
        (
            Bound::Excluded(events_ids[0].clone()),
            Bound::Excluded(events_ids[4].clone()),
            3,
            1..4,
        ),
        // Mixed Included/Excluded bounds
        (
            Bound::Included(events_ids[1].clone()),
            Bound::Excluded(events_ids[4].clone()),
            3,
            1..4,
        ),
        (
            Bound::Excluded(events_ids[0].clone()),
            Bound::Included(events_ids[3].clone()),
            3,
            1..4,
        ),
    ];

    for (start_bound, end_bound, expected_len, id_range) in tests {
        let list_return = filter_events(&mut module_impl, start_bound, end_bound);
        assert_eq!(list_return.nb_events, 5);
        assert_eq!(list_return.events.len(), expected_len);

        list_return
            .events
            .into_iter()
            .zip(events_ids[id_range].iter())
            .for_each(|(event, id)| {
                assert_eq!(event.id, *id);
            });
    }
}

fn get_all_events_ids(module_impl: &mut LedgerModuleImpl) -> Vec<EventId> {
    let result = module_impl
        .list(events::ListArgs {
            count: None,
            order: None,
            filter: None,
        })
        .unwrap();

    assert_eq!(result.nb_events, 5);
    assert_eq!(result.events.len(), 5);

    result.events.iter().map(|e| e.id.clone()).collect()
}

fn filter_events(
    module_impl: &mut LedgerModuleImpl,
    start_bound: Bound<EventId>,
    end_bound: Bound<EventId>,
) -> events::ListReturns {
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            id_range: Some(CborRange {
                start: start_bound,
                end: end_bound,
            }),
            ..events::EventFilter::default()
        }),
    });

    assert!(result.is_ok());

    result.unwrap()
}

#[test]
fn list_invalid_filter_id() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();

    // Create 5 events
    for i in 0..5 {
        send(&mut module_impl, id, identity(i));
    }
    let invalid_tests = vec![
        // Hypothetical error case - included range out of bounds
        (
            Bound::Included(vec![6, 7, 8].into()),
            Bound::Included(vec![9, 10, 11].into()),
        ),
        // Hypothetical error case - excluded range out of bounds
        (
            Bound::Excluded(vec![6, 7, 8].into()),
            Bound::Excluded(vec![9, 10, 11].into()),
        ),
        // Hypothetical error case - start included and end excluded, but range is out of bounds
        (
            Bound::Included(vec![0].into()),
            Bound::Excluded(vec![10].into()),
        ),
        // Hypothetical error case - start excluded and end included, but range is out of bounds
        (
            Bound::Excluded(vec![0].into()),
            Bound::Included(vec![10].into()),
        ),
        // Hypothetical error case - start unbounded and end included, but range is out of bounds
        (Bound::Unbounded, Bound::Included(vec![6].into())),
        // Hypothetical error case - start included and end unbounded, but range is out of bounds
        (Bound::Included(vec![0].into()), Bound::Unbounded),
        // Hypothetical error case - start unbounded and end excluded, but range is out of bounds
        (Bound::Unbounded, Bound::Excluded(vec![6].into())),
        // Hypothetical error case - start excluded and end unbounded, but range is out of bounds
        (Bound::Excluded(vec![0].into()), Bound::Unbounded),
    ];

    for (start_bound, end_bound) in invalid_tests {
        assert_invalid_filter(&mut module_impl, start_bound, end_bound);
    }
}

fn assert_invalid_filter(
    module_impl: &mut LedgerModuleImpl,
    start_bound: Bound<EventId>,
    end_bound: Bound<EventId>,
) {
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            id_range: Some(CborRange {
                start: start_bound,
                end: end_bound,
            }),
            ..events::EventFilter::default()
        }),
    });

    assert!(result.is_err());
}
#[test]
fn list_filter_date() {
    let Setup {
        mut module_impl,
        id,
        ..
    } = setup();
    let before = Timestamp::now();
    send(&mut module_impl, id, identity(1));
    // TODO: Remove this when we support factional seconds
    // See https://github.com/liftedinit/many-rs/issues/110
    std::thread::sleep(std::time::Duration::new(1, 0));
    let after = Timestamp::now();
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            date_range: Some(CborRange {
                start: Bound::Included(before),
                end: Bound::Included(after),
            }),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
    assert_eq!(list_return.events[0].kind(), events::EventKind::Send);
    assert!(list_return.events[0].is_about(*MFX_SYMBOL));
    assert!(list_return.events[0].is_about(id));

    // TODO: Remove this when we support factional seconds
    // See https://github.com/liftedinit/many-rs/issues/110
    std::thread::sleep(std::time::Duration::new(1, 0));
    let now = Timestamp::now();
    let result = module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            date_range: Some(CborRange {
                start: Bound::Included(now),
                end: Bound::Unbounded,
            }),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.events.len(), 0);
}

fn submit_args(
    account_id: Address,
    transaction: events::AccountMultisigTransaction,
    execute_automatically: Option<bool>,
) -> multisig::SubmitTransactionArgs {
    multisig::SubmitTransactionArgs {
        account: account_id,
        memo: Some(Memo::try_from("Foo".to_string()).unwrap()),
        transaction: Box::new(transaction),
        threshold: None,
        timeout_in_secs: None,
        execute_automatically,
        data_: None,
        memo_: None,
    }
}

proptest! {
    #![proptest_config(Config {cases: 200, source_file: Some("tests/events"), .. Config::default()})]

    // TODO test more MultiSigTransactionState variants
    #[test]
    fn list_filter_attribute_specific(SetupWithAccountAndTx {
        mut module_impl,
        id,
        account_id,
        tx,
    } in setup_with_account_and_tx(AccountType::Multisig)) {
        let submit_args = submit_args(account_id, tx, None);
        module_impl
            .multisig_submit_transaction(&id, submit_args)
            .expect("Multisig transaction should be sent");

        let result = module_impl.list(events::ListArgs {
            count: None,
            order: None,
            filter: Some(events::EventFilter{
                events_filter_attribute_specific: BTreeMap::from([
                    (EventFilterAttributeSpecificIndex::MultisigTransactionState,
                     EventFilterAttributeSpecific::MultisigTransactionState(vec![MultisigTransactionState::Pending].into()))
                ]),
                ..events::EventFilter::default()
            })
        }).expect("List should return a value");

        assert!(!result.events.is_empty());

        let result = module_impl.list(events::ListArgs {
            count: None,
            order: None,
            filter: Some(events::EventFilter{
                events_filter_attribute_specific: BTreeMap::from([
                    (EventFilterAttributeSpecificIndex::MultisigTransactionState,
                     EventFilterAttributeSpecific::MultisigTransactionState(vec![MultisigTransactionState::Withdrawn].into()))
                ]),
                ..events::EventFilter::default()
            })
        }).expect("List should return a value");
        assert!(result.events.is_empty());
    }
}
