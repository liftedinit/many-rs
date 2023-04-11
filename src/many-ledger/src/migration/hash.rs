use {
    crate::{
        migration::{InnerMigration, MIGRATIONS},
        storage::{v1_forest, v2_forest, InnerStorage, Operation},
    },
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
                        //.map_err(ManyError::unknown)?
                        .unwrap()
                        .as_slice(),
                )
                //.map_err(ManyError::unknown)?;
                .unwrap();
            replacement.commit(&[]).unwrap(); //.map_err(ManyError::unknown)?;
            core::mem::replace(
                merk,
                merk_v1::Merk::open([""].iter().collect::<std::path::PathBuf>())
                    //.map_err(ManyError::unknown)?,
                    .unwrap(),
            )
            .destroy()
            //.map_err(ManyError::unknown)?
            .unwrap()
        }
        InnerStorage::V2(_) => (),
    }
    let mut destination = InnerStorage::open_v2(path).unwrap(); //.map_err(ManyError::unknown)?;
    match replacement {
        InnerStorage::V1(_) => (),
        InnerStorage::V2(ref merk) => {
            destination
                .apply(
                    v2_forest(merk, IteratorMode::Start, Default::default())
                        .map(|key_value_pair| {
                            key_value_pair.map(|(key, value)| {
                                (key, Operation::from(Op::Put(value.value().to_vec())))
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                        //.map_err(ManyError::unknown)?
                        .unwrap()
                        .as_slice(),
                )
                //.map_err(ManyError::unknown)?;
                .unwrap();
            destination.commit(&[]).unwrap(); //.map_err(ManyError::unknown)?;
        }
    }
    *storage = destination;
    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static HASH_MIGRATION: InnerMigration<InnerStorage, ManyError> = InnerMigration::new_hash(
    initialize,
    "Hash Migration",
    "Move data from old version of merk hash scheme to new version of merk hash scheme",
);
