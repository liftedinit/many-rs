use {
    crate::{
        migration::{InnerMigration, MIGRATIONS},
        storage::{v1_forest, InnerStorage, Operation},
    },
    linkme::distributed_slice,
    many_error::ManyError,
    merk_v1::rocksdb::IteratorMode,
    merk_v2::Op,
};

fn initialize(storage: &mut InnerStorage, mut replacement: InnerStorage) -> Result<(), ManyError> {
    match storage {
        InnerStorage::V1(merk) => {
            replacement
                .apply(
                    v1_forest(merk, IteratorMode::Start, Default::default())
                        .map(|key_value_pair| {
                            key_value_pair.map(|(key, value)| {
                                (key, Operation::from(Op::Put(value.value().to_vec())))
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(ManyError::unknown)?
                        .as_slice(),
                )
                .map_err(ManyError::unknown)?;
            replacement.commit(&[]).map_err(ManyError::unknown)?;
            *storage = replacement;
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
