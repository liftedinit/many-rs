pub mod common;

use common::*;
use many_identity::testing::identity;
use many_modules::events;
use many_modules::events::EventsModuleBackend;
use many_types::{CborRange, Timestamp};
use std::ops::Bound;

#[test]
fn events() {
    let mut setup = setup();
    let id = setup.id;
    let result = events::EventsModuleBackend::info(&setup.module_impl, events::InfoArgs {});
    assert!(result.is_ok());
    assert_eq!(result.unwrap().total, 0);
    setup
        .put(&id, vec![10, 11, 12], vec![4, 5, 6], None)
        .unwrap();
    let result = events::EventsModuleBackend::info(&setup.module_impl, events::InfoArgs {});
    assert!(result.is_ok());
    assert_eq!(result.unwrap().total, 1);
}

#[test]
fn list() {
    let mut setup = setup();
    let id = setup.id;
    setup
        .put(&id, vec![11, 11, 12], vec![4, 5, 6], None)
        .unwrap();
    let result = setup.module_impl.list(events::ListArgs {
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
fn list_filter_account() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let account_id = setup.account_id;
    setup
        .put(
            &identity(2),
            vec![11, 11, 22],
            vec![4, 5, 6],
            Some(account_id),
        )
        .unwrap();
    setup
        .put(&identity(1), vec![11, 11, 23], vec![44, 55, 66], None)
        .unwrap();
    let result = setup.module_impl().list(events::ListArgs {
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
            events::EventInfo::KvStorePut { owner, .. } => {
                assert_eq!(owner, account_id);
            }
            _ => unimplemented!(),
        }
    }
}

#[test]
fn list_filter_kind() {
    let mut setup = setup();
    let id = setup.id;
    setup
        .put(&id, vec![11, 21, 23], vec![44, 55, 66], None)
        .unwrap();
    let result = setup.module_impl.list(events::ListArgs {
        count: None,
        order: None,
        filter: Some(events::EventFilter {
            kind: Some(vec![events::EventKind::KvStorePut].into()),
            ..events::EventFilter::default()
        }),
    });
    assert!(result.is_ok());
    let list_return = result.unwrap();
    assert_eq!(list_return.nb_events, 1);
    assert_eq!(list_return.events.len(), 1);
    assert_eq!(list_return.events[0].kind(), events::EventKind::KvStorePut);
    assert!(list_return.events[0].is_about(id));
}

#[test]
fn list_filter_date() {
    let mut setup = setup();
    let id = setup.id;
    let before = Timestamp::now();
    setup
        .put(&id, vec![11, 21, 23], vec![44, 55, 66], None)
        .unwrap();
    // TODO: Remove this when we support factional seconds
    // See https://github.com/liftedinit/many-rs/issues/110
    let after = before + 1;
    let result = setup.module_impl.list(events::ListArgs {
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
    assert_eq!(list_return.events[0].kind(), events::EventKind::KvStorePut);
    assert!(list_return.events[0].is_about(id));

    // TODO: Remove this when we support factional seconds
    // See https://github.com/liftedinit/many-rs/issues/110
    let now = after + 1;
    let result = setup.module_impl.list(events::ListArgs {
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
