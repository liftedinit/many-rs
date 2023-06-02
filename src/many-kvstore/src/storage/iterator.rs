use many_types::SortOrder;
use merk::rocksdb;
use merk::rocksdb::{IteratorMode, ReadOptions};
use merk::tree::Tree;

pub struct KvStoreIterator<'a> {
    inner: rocksdb::DBIterator<'a>,
}

impl<'a> KvStoreIterator<'a> {
    pub fn all_keys(merk: &'a merk::Merk, order: SortOrder) -> Self {
        use crate::storage::KVSTORE_ACL_ROOT;

        // Set the iterator bounds to iterate all multisig transactions.
        let mut options = ReadOptions::default();
        options.set_iterate_range(rocksdb::PrefixRange(KVSTORE_ACL_ROOT));

        let it_mode = match order {
            SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
            SortOrder::Descending => IteratorMode::End,
        };

        let inner = merk.iter_opt(it_mode, options);

        Self { inner }
    }
}

impl<'a> Iterator for KvStoreIterator<'a> {
    type Item = Result<(Box<[u8]>, Vec<u8>), rocksdb::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| {
            item.map(|(k, v)| {
                let new_v = Tree::decode(k.to_vec(), v.as_ref());

                (k, new_v.value().to_vec())
            })
        })
    }
}
