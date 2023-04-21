use {
    clap::Parser,
    derive_more::{From, TryInto},
    many_types::ledger::TokenAmount,
    merk_v1::rocksdb::{IteratorMode, ReadOptions},
    merk_v1::tree::Tree,
    std::path::PathBuf,
};

#[derive(Parser)]
struct Opts {
    /// The RocksDB store to load.
    store: PathBuf,
}

type InnerStorage = merk_v2::Merk;

#[derive(Debug, From, TryInto)]
enum Error {
    Decode(minicbor::decode::Error),
    EndOfIterator,
    Merk(merk_v2::Error),
    Rocks(merk_v2::rocksdb::Error),
}

fn main() -> Result<(), Error> {
    let Opts { store } = Opts::parse();

    let merk = InnerStorage::open(store)?;

    let it = merk.iter_opt(IteratorMode::Start, ReadOptions::default());

    for kv_result in it {
        let (k, v) = kv_result?;
        let new_v = Tree::decode(k.to_vec(), v.as_ref());

        let k: Vec<u8> = k.into();
        let v = new_v.value();

        // Try to "smartly" decode the key.
        if k.starts_with(b"/events/") {
            let k = hex::encode(&k[8..]);
            let log = minicbor::decode::<many_modules::events::EventLog>(v)?;
            println!("event {k} => {log:?}",)
        } else if k.starts_with(b"/balances/") {
            let k = &k[10..];
            // This should be utf8.
            let k = String::from_utf8_lossy(k);
            let mut it = k.split('/');
            let (id, symbol) = it
                .next()
                .and_then(|id| it.next().map(|symbol| (id, symbol)))
                .ok_or(Error::EndOfIterator)?;
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
    Ok(())
}
