use crate::error;
use crate::migration::MIGRATIONS;
use crate::storage::data::{DATA_ATTRIBUTES_KEY, DATA_INFO_KEY};
use crate::storage::InnerStorage;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;
use many_modules::data::{DataIndex, DataInfo, DataValue};
use many_types::ledger::TokenAmount;
use merk::rocksdb::{IteratorMode, ReadOptions};
use merk::{rocksdb, Op};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

pub static ACCOUNT_TOTAL_COUNT_INDEX: DataIndex = DataIndex::new(0).with_index(2).with_index(0);
pub static NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX: DataIndex =
    DataIndex::new(0).with_index(2).with_index(1);

const BALANCES_ROOT_BYTES: &[u8] = b"/balances";

fn get_data_from_db(storage: &InnerStorage) -> Result<(u64, u64), ManyError> {
    let mut num_unique_accounts: u64 = 0;
    let mut num_non_zero_account: u64 = 0;

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(BALANCES_ROOT_BYTES));

    let iterator = storage.iter_opt(IteratorMode::Start, opts);
    for item in iterator {
        let (key, value) = item.map_err(ManyError::unknown)?; // TODO: Custom error
        let value = merk::tree::Tree::decode(key.to_vec(), value.as_ref());
        let amount = TokenAmount::from(value.value().to_vec());
        num_unique_accounts += 1;
        if !amount.is_zero() {
            num_non_zero_account += 1
        }
    }

    Ok((num_unique_accounts, num_non_zero_account))
}

fn data_info() -> BTreeMap<DataIndex, DataInfo> {
    BTreeMap::from([
        (
            ACCOUNT_TOTAL_COUNT_INDEX,
            DataInfo {
                r#type: many_modules::data::DataType::Counter,
                shortname: "accountTotalCount".to_string(),
            },
        ),
        (
            NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX,
            DataInfo {
                r#type: many_modules::data::DataType::Counter,
                shortname: "nonZeroAccountTotalCount".to_string(),
            },
        ),
    ])
}

fn data_value(
    num_unique_accounts: u64,
    num_non_zero_account: u64,
) -> BTreeMap<DataIndex, DataValue> {
    BTreeMap::from([
        (
            ACCOUNT_TOTAL_COUNT_INDEX,
            DataValue::Counter(num_unique_accounts),
        ),
        (
            NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX,
            DataValue::Counter(num_non_zero_account),
        ),
    ])
}

/// Initialize the account count data attribute
fn initialize(storage: &mut InnerStorage, _: &HashMap<String, Value>) -> Result<(), ManyError> {
    let (num_unique_accounts, num_non_zero_account) = get_data_from_db(storage)?;

    storage
        .apply(&[
            (
                DATA_ATTRIBUTES_KEY.to_vec(),
                Op::Put(
                    minicbor::to_vec(data_value(num_unique_accounts, num_non_zero_account))
                        .map_err(ManyError::serialization_error)?,
                ),
            ),
            (
                DATA_INFO_KEY.to_vec(),
                Op::Put(minicbor::to_vec(data_info()).map_err(ManyError::serialization_error)?),
            ),
        ])
        .map_err(error::storage_apply_failed)?;
    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static ACCOUNT_COUNT_DATA_ATTRIBUTE: InnerMigration<InnerStorage, ManyError> =
    InnerMigration::new_initialize(
        initialize,
        "Account Count Data Attribute",
        r#"
            Provides the total number of unique addresses.
            Provides the total number of unique addresses with a non-zero balance.
            "#,
    );
