extern crate core;

use clap::Parser;
use merk::rocksdb;
use merk::rocksdb::{IteratorMode, ReadOptions};
use merk::tree::Tree;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(crate) const IDSTORE_ROOT: &[u8] = b"/idstore/";
pub(crate) const IDSTORE_SEED_ROOT: &[u8] = b"/config/idstore_seed";

#[derive(Parser)]
struct Opts {
    /// The RocksDB store to load.
    store: PathBuf,
}

#[derive(serde_derive::Serialize)]
struct JsonRoot {
    id_store_seed: u64,
    id_store_keys: BTreeMap<String, String>,
}

fn main() {
    let Opts { store } = Opts::parse();

    let merk = merk::Merk::open(store).expect("Could not open the store.");

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(IDSTORE_ROOT));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    let mut idstore = BTreeMap::new();
    for item in it {
        let (key, value) = item.expect("Error while reading the DB");
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        idstore.insert(base64::encode(key.as_ref()), base64::encode(value));
    }

    let root = JsonRoot {
        id_store_seed: merk
            .get(IDSTORE_SEED_ROOT)
            .expect("Could not read seed")
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            }),
        id_store_keys: idstore,
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&root).expect("Could not serialize")
    );
}
