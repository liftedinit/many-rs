use super::KvStoreStorage;
use many_modules::events;
use many_types::{CborRange, SortOrder};
use merk::tree::Tree;
use merk::{rocksdb, Op};
use std::collections::Bound;
use std::ops::RangeBounds;

const EVENTS_ROOT: &[u8] = b"/events/";

pub type EventId = events::EventId;

/// Number of bytes in an event ID when serialized. Keys smaller than this
/// will have `\0` prepended, and keys larger will be cut to this number of
/// bytes.
const EVENT_ID_KEY_SIZE_IN_BYTES: usize = 32;

/// Returns the storage key for an event in the kv-store.
fn key_for_event(id: events::EventId) -> Vec<u8> {
    let id = id.as_ref();
    let id = if id.len() > EVENT_ID_KEY_SIZE_IN_BYTES {
        &id[0..EVENT_ID_KEY_SIZE_IN_BYTES]
    } else {
        id
    };

    let mut exp_id = [0u8; EVENT_ID_KEY_SIZE_IN_BYTES];
    exp_id[(EVENT_ID_KEY_SIZE_IN_BYTES - id.len())..].copy_from_slice(id);
    vec![EVENTS_ROOT.to_vec(), exp_id.to_vec()].concat()
}

impl KvStoreStorage {
    fn new_event_id(&mut self) -> events::EventId {
        self.latest_event_id += 1;
        self.latest_event_id.clone()
    }

    pub fn nb_events(&self) -> u64 {
        self.persistent_store
            .get(b"/events_count")
            .unwrap()
            .map_or(0, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            })
    }

    pub(crate) fn log_event(&mut self, content: events::EventInfo) {
        let current_nb_events = self.nb_events();
        let event = events::EventLog {
            id: self.new_event_id(),
            time: self.now(),
            content,
        };

        self.persistent_store
            .apply(&[
                (
                    key_for_event(event.id.clone()),
                    Op::Put(minicbor::to_vec(&event).unwrap()),
                ),
                (
                    b"/events_count".to_vec(),
                    Op::Put((current_nb_events + 1).to_be_bytes().to_vec()),
                ),
            ])
            .unwrap();

        if !self.blockchain {
            self.persistent_store.commit(&[]).unwrap();
        }
    }

    pub fn iter(&self, range: CborRange<events::EventId>, order: SortOrder) -> KvStoreIterator {
        KvStoreIterator::scoped_by_id(&self.persistent_store, range, order)
    }
}

pub struct KvStoreIterator<'a> {
    inner: rocksdb::DBIterator<'a>,
}

impl<'a> KvStoreIterator<'a> {
    pub fn scoped_by_id(
        merk: &'a merk::Merk,
        range: CborRange<events::EventId>,
        order: SortOrder,
    ) -> Self {
        use rocksdb::{IteratorMode, ReadOptions};
        let mut opts = ReadOptions::default();

        match range.start_bound() {
            Bound::Included(x) => opts.set_iterate_lower_bound(key_for_event(x.clone())),
            Bound::Excluded(x) => opts.set_iterate_lower_bound(key_for_event(x.clone() + 1)),
            Bound::Unbounded => opts.set_iterate_lower_bound(EVENTS_ROOT),
        }
        match range.end_bound() {
            Bound::Included(x) => opts.set_iterate_upper_bound(key_for_event(x.clone() + 1)),
            Bound::Excluded(x) => opts.set_iterate_upper_bound(key_for_event(x.clone())),
            Bound::Unbounded => {
                let mut bound = EVENTS_ROOT.to_vec();
                bound[EVENTS_ROOT.len() - 1] += 1;
                opts.set_iterate_upper_bound(bound);
            }
        }

        let mode = match order {
            SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
            SortOrder::Descending => IteratorMode::End,
        };

        Self {
            inner: merk.iter_opt(mode, opts),
        }
    }
}

impl<'a> Iterator for KvStoreIterator<'a> {
    type Item = Result<(Box<[u8]>, Vec<u8>), merk::rocksdb::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| {
            item.map(|(k, v)| {
                let new_v = Tree::decode(k.to_vec(), v.as_ref());

                (k, new_v.value().to_vec())
            })
        })
    }
}
