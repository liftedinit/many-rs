extern crate core;

use {
    base64::{engine::general_purpose, Engine as _},
    clap::Parser,
    derive_more::{From, TryInto},
    merk_v2::rocksdb::{self, IteratorMode, ReadOptions},
    merk_v2::tree::Tree,
    std::collections::BTreeMap,
    std::path::PathBuf,
};

pub(crate) const IDSTORE_ROOT: &[u8] = b"/idstore/";
pub(crate) const IDSTORE_SEED_ROOT: &[u8] = b"/config/idstore_seed";

type InnerStorage = merk_v2::Merk;

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

#[derive(Debug, From, TryInto)]
enum Error {
    Merk(merk_v2::Error),
    Rocks(merk_v2::rocksdb::Error),
    Serde(serde_json::Error),
}

fn main() -> Result<(), Error> {
    let Opts { store } = Opts::parse();

    let merk = InnerStorage::open(store)?;

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(IDSTORE_ROOT));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    let mut idstore = BTreeMap::new();
    for item in it {
        let (key, value) = item?;
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        idstore.insert(
            general_purpose::STANDARD.encode(key.as_ref()),
            general_purpose::STANDARD.encode(value),
        );
    }

    let root = JsonRoot {
        id_store_seed: merk.get(IDSTORE_SEED_ROOT)?.map_or(0u64, |x| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(x.as_slice());
            u64::from_be_bytes(bytes)
        }),
        id_store_keys: idstore,
    };

    println!("{}", serde_json::to_string_pretty(&root)?);
    Ok(())
}
