use {
    crate::storage::event::{key_for_event, EVENTS_ROOT},
    crate::storage::InnerStorage,
    many_modules::events::EventId,
    many_types::{CborRange, SortOrder},
    merk_v2::{rocksdb, tree::Tree},
    rocksdb::IteratorMode,
    std::collections::Bound,
    std::ops::RangeBounds,
};

pub struct LedgerIterator<'a> {
    inner: rocksdb::DBIterator<'a>,
}

impl<'a> LedgerIterator<'a> {
    pub fn all_multisig(merk: &'a InnerStorage, order: SortOrder) -> Self {
        use crate::storage::multisig::MULTISIG_TRANSACTIONS_ROOT;

        let inner = match merk {
            InnerStorage::V1(_) => {
                let it_mode = match order {
                    SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
                    SortOrder::Descending => IteratorMode::End,
                };

                merk.iter_opt(it_mode, {
                    // Set the iterator bounds to iterate all multisig transactions.
                    let mut options = merk_v1::rocksdb::ReadOptions::default();
                    options.set_iterate_range(rocksdb::PrefixRange(MULTISIG_TRANSACTIONS_ROOT));
                    options
                })
            }
            InnerStorage::V2(_) => {
                let it_mode = match order {
                    SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
                    SortOrder::Descending => IteratorMode::End,
                };

                merk.iter_opt(it_mode, {
                    // Set the iterator bounds to iterate all multisig transactions.
                    let mut options = merk_v2::rocksdb::ReadOptions::default();
                    options.set_iterate_range(rocksdb::PrefixRange(MULTISIG_TRANSACTIONS_ROOT));
                    options
                })
            }
        };

        Self { inner }
    }

    pub fn all_symbols(merk: &'a InnerStorage, order: SortOrder) -> Self {
        use crate::storage::ledger_tokens::SYMBOLS_ROOT_DASH;

        let inner = match merk {
            InnerStorage::V1(_) => {
                let it_mode = match order {
                    SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
                    SortOrder::Descending => IteratorMode::End,
                };

                merk.iter_opt(it_mode, {
                    let mut options = merk_v1::rocksdb::ReadOptions::default();
                    options.set_iterate_range(rocksdb::PrefixRange(SYMBOLS_ROOT_DASH.as_bytes()));
                    options
                })
            }
            InnerStorage::V2(_) => {
                let it_mode = match order {
                    SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
                    SortOrder::Descending => IteratorMode::End,
                };

                merk.iter_opt(it_mode, {
                    let mut options = merk_v2::rocksdb::ReadOptions::default();
                    options.set_iterate_range(rocksdb::PrefixRange(SYMBOLS_ROOT_DASH.as_bytes()));
                    options
                })
            }
        };

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
        Self {
            inner: match merk {
                InnerStorage::V1(_) => {
                    let mut opts = merk_v1::rocksdb::ReadOptions::default();

                    match range.start_bound() {
                        Bound::Included(x) => {
                            opts.set_iterate_lower_bound(key_for_event(x.clone()))
                        }
                        Bound::Excluded(x) => {
                            opts.set_iterate_lower_bound(key_for_event(x.clone() + 1))
                        }
                        Bound::Unbounded => opts.set_iterate_lower_bound(EVENTS_ROOT),
                    }
                    match range.end_bound() {
                        Bound::Included(x) => {
                            opts.set_iterate_upper_bound(key_for_event(x.clone() + 1))
                        }
                        Bound::Excluded(x) => {
                            opts.set_iterate_upper_bound(key_for_event(x.clone()))
                        }
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
                    merk.iter_opt(mode, opts)
                }
                InnerStorage::V2(_) => {
                    let mut opts = merk_v2::rocksdb::ReadOptions::default();

                    match range.start_bound() {
                        Bound::Included(x) => {
                            opts.set_iterate_lower_bound(key_for_event(x.clone()))
                        }
                        Bound::Excluded(x) => {
                            opts.set_iterate_lower_bound(key_for_event(x.clone() + 1))
                        }
                        Bound::Unbounded => opts.set_iterate_lower_bound(EVENTS_ROOT),
                    }
                    match range.end_bound() {
                        Bound::Included(x) => {
                            opts.set_iterate_upper_bound(key_for_event(x.clone() + 1))
                        }
                        Bound::Excluded(x) => {
                            opts.set_iterate_upper_bound(key_for_event(x.clone()))
                        }
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
                    merk.iter_opt(mode, opts)
                }
            },
        }
    }
}

impl<'a> Iterator for LedgerIterator<'a> {
    type Item = Result<(Box<[u8]>, Vec<u8>), merk_v2::rocksdb::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| {
            item.map(|(k, v)| {
                let new_v = Tree::decode(k.to_vec(), v.as_ref());

                (k, new_v.value().to_vec())
            })
        })
    }
}
