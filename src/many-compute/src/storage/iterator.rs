use many_identity::Address;
use many_types::SortOrder;
use merk::rocksdb;
use merk::rocksdb::{IteratorMode, ReadOptions};
use merk::tree::Tree;

pub struct ComputeIterator<'a> {
    inner: rocksdb::DBIterator<'a>,
}

impl<'a> ComputeIterator<'a> {
    pub fn all_dseq(
        merk: &'a merk::Merk,
        order: Option<SortOrder>,
        owner: Option<Address>,
    ) -> Self {
        // Set the iterator bounds to iterate all multisig transactions.
        let mut options = ReadOptions::default();
        let prefix = owner.map_or("/deploy/".to_string().into_bytes(), |owner| {
            format!("/deploy/{owner}").into_bytes()
        });
        options.set_iterate_range(rocksdb::PrefixRange(prefix));

        let it_mode = match order.unwrap_or_default() {
            SortOrder::Indeterminate | SortOrder::Ascending => IteratorMode::Start,
            SortOrder::Descending => IteratorMode::End,
        };

        let inner = merk.iter_opt(it_mode, options);

        Self { inner }
    }
}

impl<'a> Iterator for ComputeIterator<'a> {
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
