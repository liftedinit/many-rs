use many_identity::testing::identity;
use many_identity::Address;
use many_ledger::storage::LedgerStorage;
use many_modules::events::{EventId, EventLog};
use many_types::ledger::TokenAmount;
use many_types::{CborRange, SortOrder};
use std::collections::BTreeMap;
use std::ops::Bound;

fn setup() -> LedgerStorage {
    let symbol0 = Address::anonymous();
    let id0 = identity(0);
    let id1 = identity(1);
    let id2 = identity(2);

    let symbols = BTreeMap::from_iter(vec![(symbol0, "MFX".to_string())].into_iter());
    let balances = BTreeMap::from([(id0, BTreeMap::from([(symbol0, TokenAmount::from(1000u16))]))]);
    let persistent_path = tempfile::tempdir().unwrap();

    let mut storage = LedgerStorage::new(&symbols, persistent_path, id2, false)
        .unwrap()
        .with_balances(&symbols, &balances)
        .unwrap()
        .build()
        .unwrap();

    for _ in 0..5 {
        storage
            .send(&id0, &id1, &symbol0, TokenAmount::from(100u16), None)
            .unwrap();
    }

    // Check that we have 5 events (5 sends).
    assert_eq!(storage.nb_events().unwrap(), 5);

    storage
}

fn iter_asc(
    storage: &LedgerStorage,
    start: Bound<EventId>,
    end: Bound<EventId>,
) -> impl Iterator<Item = EventLog> + '_ {
    storage
        .iter_events(CborRange { start, end }, SortOrder::Ascending)
        .map(|item| {
            let (_, v) = item.expect("Error while reading DB");
            minicbor::decode(&v).expect("Iterator item not an event.")
        })
}

#[test]
fn range_works() {
    let storage = setup();

    // Get the first event ID.
    let mut iter = iter_asc(&storage, Bound::Unbounded, Bound::Unbounded);
    let first_ev = iter.next().expect("No events?");
    let first_id = first_ev.id;
    let last_ev = iter.last().expect("Only 1 event");
    let last_id = last_ev.id;

    // Make sure exclusive range removes the first_id.
    assert!(iter_asc(
        &storage,
        Bound::Excluded(first_id.clone()),
        Bound::Unbounded
    )
    .all(|x| x.id != first_id));

    let iter = iter_asc(
        &storage,
        Bound::Excluded(first_id.clone()),
        Bound::Unbounded,
    );
    assert_eq!(iter.last().expect("Should have a last item").id, last_id);

    // Make sure exclusive range removes the last_id.
    assert!(
        iter_asc(&storage, Bound::Unbounded, Bound::Excluded(last_id.clone()))
            .all(|x| x.id != last_id)
    );

    let mut iter = iter_asc(&storage, Bound::Unbounded, Bound::Excluded(last_id.clone()));
    assert_eq!(iter.next().expect("Should have a first item").id, first_id);

    // Make sure inclusive bounds include first_id.
    let mut iter = iter_asc(
        &storage,
        Bound::Included(first_id.clone()),
        Bound::Included(last_id.clone()),
    );
    assert_eq!(iter.next().expect("Should have a first item").id, first_id);
    assert_eq!(iter.last().expect("Should have a last item").id, last_id);
}
