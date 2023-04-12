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
    std::path::{Path, PathBuf},
};

fn initialize<P: AsRef<Path>>(storage: &mut InnerStorage, path: P) -> Result<(), ManyError> {
    match storage {
        InnerStorage::V1(merk) => v1_forest(merk, IteratorMode::Start, Default::default())
            .map(|key_value_pair| {
                key_value_pair
                    .map(|(key, value)| (key, Operation::from(Op::Put(value.value().to_vec()))))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(ManyError::unknown)
            .and_then(|trees| {
                InnerStorage::open_v2(["/tmp", "temp1"].iter().collect::<PathBuf>())
                    .map(|replacement| (trees, replacement))
                    .map_err(Into::into)
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
                merk_v1::Merk::open(["/tmp", "temp2"].iter().collect::<PathBuf>())
                    .map_err(ManyError::unknown)
                    .map(|new_storage| (new_storage, replacement))
            })
            .and_then(|(new_storage, replacement)| {
                replace(merk, new_storage)
                    .destroy()
                    .map_err(ManyError::unknown)
                    .map(|_| replacement)
            }),
        InnerStorage::V2(_) => {
            InnerStorage::open_v2(["/tmp", "temp1"].iter().collect::<PathBuf>()).map_err(Into::into)
        }
    }
    .and_then(|replacement| {
        InnerStorage::open_v2(path.as_ref().to_path_buf())
            .map_err(ManyError::unknown)
            .map(|destination| (replacement, destination))
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
