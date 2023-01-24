use crate::error;
use crate::error::storage_commit_failed;
use crate::migration::MIGRATIONS;
use crate::storage::iterator::LedgerIterator;
use crate::storage::multisig::MultisigTransactionStorage;
use crate::storage::InnerStorage;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;
use many_modules::account::features::multisig::InfoReturn;
use many_modules::events::{EventInfo, EventLog};
use many_types::{Memo, SortOrder};
use merk::Op;
use serde_json::Value;
use std::borrow::BorrowMut;
use std::collections::HashMap;

fn iter_through_events(
    storage: &InnerStorage,
) -> impl Iterator<Item = Result<(Vec<u8>, EventLog), ManyError>> + '_ {
    LedgerIterator::all_events(storage).map(|r| match r {
        Ok((k, v)) => {
            let log = minicbor::decode::<EventLog>(v.as_slice())
                .map_err(ManyError::deserialization_error)?;
            Ok((k.into(), log))
        }
        Err(e) => Err(ManyError::unknown(e)),
    })
}

fn iter_through_multisig_storage(
    storage: &InnerStorage,
) -> impl Iterator<Item = Result<(Vec<u8>, MultisigTransactionStorage), ManyError>> + '_ {
    LedgerIterator::all_multisig(storage, SortOrder::Ascending).map(|r| match r {
        Ok((k, v)) => {
            let log = minicbor::decode::<MultisigTransactionStorage>(v.as_slice())
                .map_err(ManyError::deserialization_error)?;
            Ok((k.into(), log))
        }
        Err(e) => Err(ManyError::unknown(e)),
    })
}

fn update_multisig_submit_events(storage: &mut InnerStorage) -> Result<(), ManyError> {
    let mut batch = Vec::new();

    for log in iter_through_events(storage) {
        let (key, EventLog { id, time, content }) = log?;

        if let EventInfo::AccountMultisigSubmit {
            submitter,
            account,
            memo_,
            transaction,
            token,
            threshold,
            timeout,
            execute_automatically,
            data_,
            memo,
        } = content
        {
            if memo.is_some() {
                continue;
            }
            let memo = match (memo_, data_) {
                (Some(m), Some(d)) => {
                    let mut m = Memo::from(m);
                    m.push_bytes(d.as_bytes().to_vec())?;
                    Some(m)
                }
                (Some(m), _) => Some(Memo::from(m)),
                (_, Some(d)) => Some(Memo::from(d)),
                _ => None,
            };

            if let Some(memo) = memo {
                let new_log = EventLog {
                    id,
                    time,
                    content: EventInfo::AccountMultisigSubmit {
                        submitter,
                        account,
                        memo_: None,
                        transaction,
                        token,
                        threshold,
                        timeout,
                        execute_automatically,
                        data_: None,
                        memo: Some(memo),
                    },
                };
                batch.push((
                    key,
                    Op::Put(minicbor::to_vec(new_log).map_err(ManyError::serialization_error)?),
                ));
            }
        }
    }

    // The iterator is already sorted when going through rocksdb.
    // Since we only filter and map above, the keys in batch will always
    // be sorted at this point.
    storage
        .apply(batch.as_slice())
        .map_err(error::storage_apply_failed)?;
    storage.commit(&[]).map_err(storage_commit_failed)?;
    Ok(())
}

fn update_multisig_storage(storage: &mut InnerStorage) -> Result<(), ManyError> {
    let mut batch = Vec::new();

    for multisig in iter_through_multisig_storage(storage) {
        let (
            key,
            MultisigTransactionStorage {
                account,
                info,
                creation,
                disabled,
            },
        ) = multisig?;

        if info.memo.is_some() {
            continue;
        }

        let new_memo = match (info.memo_, info.data_) {
            (Some(m), Some(d)) => {
                let mut memo = Memo::from(m);
                memo.push_bytes(d.as_bytes().to_vec())?;
                Some(memo)
            }
            (Some(m), _) => Some(Memo::from(m)),
            (_, Some(d)) => Some(Memo::from(d)),
            _ => None,
        };

        if let Some(memo) = new_memo {
            let new_multisig = MultisigTransactionStorage {
                account,
                creation,
                info: InfoReturn {
                    memo_: None,
                    data_: None,
                    memo: Some(memo),
                    ..info
                },
                disabled,
            };

            batch.push((
                key,
                Op::Put(minicbor::to_vec(new_multisig).map_err(ManyError::serialization_error)?),
            ));
        }
    }

    // The iterator is already sorted when going through rocksdb.
    // Since we only filter and map above, the keys in batch will always
    // be sorted at this point.
    storage
        .apply(batch.as_slice())
        .map_err(error::storage_apply_failed)?;
    storage.commit(&[]).map_err(storage_commit_failed)?;
    Ok(())
}

fn initialize(storage: &mut InnerStorage, _: &HashMap<String, Value>) -> Result<(), ManyError> {
    update_multisig_submit_events(storage.borrow_mut())?;
    update_multisig_storage(storage)?;
    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static MEMO_MIGRATION: InnerMigration<InnerStorage, ManyError> = InnerMigration::new_initialize(
    initialize,
    "Memo Migration",
    "Move the database from legacy memo and data to the new memo data type.",
);
