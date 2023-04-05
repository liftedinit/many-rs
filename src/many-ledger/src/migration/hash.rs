use {
    crate::{
        migration::{InnerMigration, MIGRATIONS},
        storage::{InnerStorage, Operation, SYMBOLS_ROOT},
    },
    linkme::distributed_slice,
    many_error::ManyError,
    many_types::ledger::Symbol,
    merk_v1::rocksdb::{IteratorMode, ReadOptions},
    merk_v2::Op,
    std::collections::BTreeMap,
};

fn initialize(storage: &mut InnerStorage, mut replacement: InnerStorage) -> Result<(), ManyError> {
    let root = minicbor::decode::<BTreeMap<Symbol, String>>(
        storage
            .get(SYMBOLS_ROOT.as_bytes())
            .unwrap()
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    println!("Old SYMBOLS_ROOT: {root:?}");
    match storage {
        InnerStorage::V1(merk) => {
            replacement
                .apply(
                    merk.iter_opt(IteratorMode::Start, ReadOptions::default())
                        .map(|key_value_pair| {
                            key_value_pair.map(|(key, value)| {
                                (key.to_vec(), Operation::from(Op::Put(value.to_vec())))
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(ManyError::unknown)?
                        .as_slice(),
                )
                .map_err(ManyError::unknown)?;
            replacement.commit(&[]).map_err(ManyError::unknown)?;
            *storage = replacement;
            let root = minicbor::decode::<BTreeMap<Symbol, String>>(
                storage
                    .get(SYMBOLS_ROOT.as_bytes())
                    .unwrap()
                    .unwrap()
                    .as_slice(),
            )
            .unwrap();
            println!("SYMBOLS_ROOT: {root:?}");
        }
        InnerStorage::V2(_) => (),
    }
    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static HASH_MIGRATION: InnerMigration<InnerStorage, ManyError> = InnerMigration::new_hash(
    initialize,
    "Hash Migration",
    "Move data from old version of merk hash scheme to new version of merk hash scheme",
);
