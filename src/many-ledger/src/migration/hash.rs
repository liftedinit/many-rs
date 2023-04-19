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
    serde_json::Value,
    std::collections::HashMap,
    std::path::{Path, PathBuf},
    tempfile::tempdir,
};

fn initialize<P: AsRef<Path>>(
    storage: &mut InnerStorage,
    _: P,
    extra: &HashMap<String, Value>,
) -> Result<(), ManyError> {
    match storage {
        InnerStorage::V1(merk) => v1_forest(merk, IteratorMode::Start, Default::default())
            .map(|key_value_pair| {
                key_value_pair
                    .map(|(key, value)| (key, Operation::from(Op::Put(value.value().to_vec()))))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(ManyError::unknown)
            .and_then(|trees| {
                tempdir()
                    .map_err(ManyError::unknown)
                    .map(|dir| dir.path().join("temp1"))
                    .and_then(|file| {
                        InnerStorage::open_v2(file)
                            .map(|replacement| (trees, replacement))
                            .map_err(Into::into)
                    })
            })
            .and_then(|(trees, mut replacement)| {
                replacement
                    .apply(trees.as_slice())
                    .map_err(Into::into)
                    .map(|_| replacement)
            })
            .and_then(|mut replacement| {
                replacement
                    .commit(&[])
                    .map_err(Into::into)
                    .map(|_| replacement)
            })
            .and_then(|replacement| {
                tempdir()
                    .map_err(ManyError::unknown)
                    .map(|dir| dir.path().join("temp1"))
                    .and_then(|file| {
                        merk_v1::Merk::open(file)
                            .map_err(ManyError::unknown)
                            .map(|new_storage| (new_storage, replacement))
                    })
            })
            .and_then(|(new_storage, replacement)| {
                replace(merk, new_storage)
                    .destroy()
                    .map_err(ManyError::unknown)
                    .map(|_| replacement)
            }),
        InnerStorage::V2(_) => tempdir()
            .map_err(ManyError::unknown)
            .map(|dir| dir.path().join("temp1"))
            .and_then(|file| InnerStorage::open_v2(file).map_err(Into::into)),
    }
    .and_then(|replacement| {
        extra
            .get("ledger-db-path")
            .ok_or_else(|| ManyError::unknown("Missing ledger db path"))
            .and_then(|db_path| {
                InnerStorage::open_v2(db_path.to_string())
                    .map_err(ManyError::unknown)
                    .map(|destination| (replacement, destination))
            })
    })
    .and_then(|(replacement, mut destination)| match replacement {
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
                destination.commit(&[]).map_err(Into::into).map(|_| {
                    *storage = destination;
                })
            }),
    })
}

#[distributed_slice(MIGRATIONS)]
pub static HASH_MIGRATION: InnerMigration<InnerStorage, ManyError, PathBuf> =
    InnerMigration::new_hash(
        initialize,
        "Hash Migration",
        "Move data from old version of merk hash scheme to new version of merk hash scheme",
    );
