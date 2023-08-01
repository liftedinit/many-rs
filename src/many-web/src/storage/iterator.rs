use crate::storage::{key_for_website, META_ROOT};
use many_identity::Address;
use many_types::SortOrder;
use merk::rocksdb;
use merk::rocksdb::{IteratorMode, ReadOptions};
use merk::tree::Tree;

pub struct WebIterator<'a> {
    inner: rocksdb::DBIterator<'a>,
}

impl<'a> WebIterator<'a> {
    pub fn meta(merk: &'a merk::Merk, order: SortOrder) -> Self {
        let mut options = ReadOptions::default();

        options.set_iterate_range(rocksdb::PrefixRange(META_ROOT));

        let mode = match order {
            SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
            SortOrder::Descending => IteratorMode::End,
        };

        let inner = merk.iter_opt(mode, options);

        Self { inner }
    }

    pub fn website_files<S: AsRef<str>>(
        merk: &'a merk::Merk,
        owner: &Address,
        site_name: &S,
    ) -> Self {
        let mut options = ReadOptions::default();
        options.set_iterate_range(rocksdb::PrefixRange(key_for_website(
            owner,
            site_name.as_ref(),
        )));

        let inner = merk.iter_opt(IteratorMode::Start, options);

        Self { inner }
    }
}

impl<'a> Iterator for WebIterator<'a> {
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
