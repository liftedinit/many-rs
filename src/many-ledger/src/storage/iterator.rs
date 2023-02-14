use crate::storage::event::{key_for_event, EVENTS_ROOT};
use crate::storage::InnerStorage;
use many_modules::events::EventId;
use many_types::{CborRange, SortOrder};
use merk::rocksdb;
use merk::rocksdb::ReadOptions;
use merk::tree::Tree;
use rocksdb::IteratorMode;
use std::collections::Bound;
use std::ops::RangeBounds;

pub struct LedgerIterator<'a> {
    inner: rocksdb::DBIterator<'a>,
}

impl<'a> LedgerIterator<'a> {
    pub fn all_multisig(merk: &'a InnerStorage, order: SortOrder) -> Self {
        use crate::storage::multisig::MULTISIG_TRANSACTIONS_ROOT;

        // Set the iterator bounds to iterate all multisig transactions.
        let mut options = ReadOptions::default();
        options.set_iterate_range(rocksdb::PrefixRange(MULTISIG_TRANSACTIONS_ROOT));

        let it_mode = match order {
            SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
            SortOrder::Descending => IteratorMode::End,
        };

        let inner = merk.iter_opt(it_mode, options);

        Self { inner }
    }

    pub fn all_symbols(merk: &'a InnerStorage, order: SortOrder) -> Self {
        use crate::storage::ledger_tokens::SYMBOLS_ROOT_DASH;

        let mut options = ReadOptions::default();
        options.set_iterate_range(rocksdb::PrefixRange(SYMBOLS_ROOT_DASH.as_bytes()));

        let it_mode = match order {
            SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
            SortOrder::Descending => IteratorMode::End,
        };

        let inner = merk.iter_opt(it_mode, options);

        Self { inner }
    }

    pub fn all_events(merk: &'a InnerStorage) -> Self {
        Self::events_scoped_by_id(merk, CborRange::default(), SortOrder::Indeterminate)
    }

    pub fn events_scoped_by_id(
        merk: &'a InnerStorage,
        range: CborRange<EventId>,
        order: SortOrder,
    ) -> Self {
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

impl<'a> Iterator for LedgerIterator<'a> {
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
