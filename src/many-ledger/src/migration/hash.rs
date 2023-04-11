use {
    crate::{
        migration::{InnerMigration, MIGRATIONS},
        storage::{v1_forest, v2_forest, InnerStorage, Operation},
    },
    core::mem::replace,
    linkme::distributed_slice,
    many_error::ManyError,
    merk_v1::rocksdb::IteratorMode,
    merk_v2::Op,
};

fn initialize(
    storage: &mut InnerStorage,
    mut replacement: InnerStorage,
    path: std::path::PathBuf,
) -> Result<(), ManyError> {
    match storage {
        InnerStorage::V1(merk) => v1_forest(merk, IteratorMode::Start, Default::default())
            .map(|key_value_pair| {
                key_value_pair
                    .map(|(key, value)| (key, Operation::from(Op::Put(value.value().to_vec()))))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(ManyError::unknown)
            .and_then(|trees| replacement.apply(trees.as_slice()).map_err(Into::into))
            .and_then(|_| replacement.commit(&[]).map_err(Into::into))
            .and_then(|_| {
                merk_v1::Merk::open(["/tmp", "temp"].iter().collect::<std::path::PathBuf>())
                    .map_err(ManyError::unknown)
            })
            .and_then(|new_storage| {
                replace(merk, new_storage)
                    .destroy()
                    .map_err(ManyError::unknown)
            }),
        InnerStorage::V2(_) => Ok(()),
    }
    .and_then(|_| InnerStorage::open_v2(path).map_err(ManyError::unknown))
    .and_then(|mut destination| match replacement {
        InnerStorage::V1(_) => {
            *storage = destination;
            Ok(())
        }
        InnerStorage::V2(ref merk) => v2_forest(merk, IteratorMode::Start, Default::default())
            .map(|key_value_pair| {
                key_value_pair
                    .map(|(key, value)| (key, Operation::from(Op::Put(value.value().to_vec()))))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(ManyError::unknown)
            .and_then(|trees| destination.apply(trees.as_slice()).map_err(Into::into))
            .and_then(|_| {
                destination
                    .commit(&[])
                    .map_err(ManyError::unknown)
                    .map(|_| {
                        *storage = destination;
                    })
            }),
    })
}

#[distributed_slice(MIGRATIONS)]
pub static HASH_MIGRATION: InnerMigration<InnerStorage, ManyError> = InnerMigration::new_hash(
    initialize,
    "Hash Migration",
    "Move data from old version of merk hash scheme to new version of merk hash scheme",
);
