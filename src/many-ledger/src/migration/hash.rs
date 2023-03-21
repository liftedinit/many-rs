use {
    crate::storage::{InnerStorage, Operation},
    many_error::ManyError,
    merk_v1::rocksdb::{IteratorMode, ReadOptions},
    merk_v2::Op,
    serde_json::Value,
    std::{collections::HashMap, path::Path},
};

fn _initialize(
    (storage, new_path): &mut (InnerStorage, Box<Path>),
    _: &HashMap<String, Value>,
) -> Result<(), ManyError> {
    match storage {
        InnerStorage::V1(merk) => {
            let mut new_storage =
                InnerStorage::open_v2(new_path.clone()).map_err(ManyError::unknown)?;
            new_storage
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
            new_storage.commit(&[]).map_err(ManyError::unknown)?;
            *storage = new_storage;
        }
        InnerStorage::V2(_) => (),
    }
    Ok(())
}
