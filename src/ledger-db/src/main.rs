use clap::Parser;
use derive_more::{From, TryInto};
use many_types::ledger::TokenAmount;
use merk_v1::rocksdb::{DBIterator, IteratorMode, ReadOptions};
use merk_v1::tree::Tree;
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Opts {
    /// The RocksDB store to load.
    store: PathBuf,
}

enum Merk {
    V1(merk_v1::Merk),
    #[allow(dead_code)]
    V2(merk_v2::Merk),
}

#[derive(Debug, From, TryInto)]
enum Error {
    V1(merk_v1::Error),
    V2(merk_v2::Error),
}

impl Merk {
    fn iter_opt(&self, mode: IteratorMode, options: ReadOptions) -> DBIterator {
        match self {
            Merk::V1(merk) => merk.iter_opt(mode, options),
            Merk::V2(merk) => merk.iter_opt(mode, options),
        }
    }

    fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        merk_v1::Merk::open(path).map(Self::V1).map_err(Into::into)
    }
}

type InnerStorage = Merk;

fn main() {
    let Opts { store } = Opts::parse();

    let merk = InnerStorage::open(store).expect("Could not open the store.");

    let it = merk.iter_opt(IteratorMode::Start, ReadOptions::default());

    for kv_result in it {
        let (k, v) = kv_result.unwrap();
        let new_v = Tree::decode(k.to_vec(), v.as_ref());

        let k: Vec<u8> = k.into();
        let v = new_v.value();

        // Try to "smartly" decode the key.
        if k.starts_with(b"/events/") {
            let k = hex::encode(&k[8..]);
            let log = minicbor::decode::<many_modules::events::EventLog>(v).unwrap();
            println!("event {k} => {log:?}",)
        } else if k.starts_with(b"/balances/") {
            let k = &k[10..];
            // This should be utf8.
            let k = String::from_utf8_lossy(k);
            let mut it = k.split('/');
            let (id, symbol) = (it.next().unwrap(), it.next().unwrap());
            let t = TokenAmount::from(v.to_vec());
            println!("balance {id} => {t} {symbol}");
        } else if k.starts_with(b"/multisig/") {
            let k = &k[10..];
            let multisig = hex::encode(v);
            println!("multisig tx 0x{} => {multisig}", hex::encode(k))
        } else if let Ok(k) = String::from_utf8(k.clone()) {
            println!("unknown {:?} => {}", k, hex::encode(v));
        } else {
            println!("unknown 0x {} => {}", hex::encode(k), hex::encode(v));
        }
    }
}
