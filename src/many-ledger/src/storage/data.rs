use crate::error;
use crate::migration::data::{ACCOUNT_TOTAL_COUNT_INDEX, NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX};
use crate::storage::{key_for_account_balance, LedgerStorage};
use many_error::ManyError;
use many_identity::Address;
use many_modules::data::{DataIndex, DataInfo, DataValue};
use many_types::ledger::TokenAmount;
use merk::Op;
use std::collections::BTreeMap;

pub const DATA_ATTRIBUTES_KEY: &[u8] = b"/data/attributes";
pub const DATA_INFO_KEY: &[u8] = b"/data/info";

impl LedgerStorage {
    pub(crate) fn data_info(&self) -> Result<Option<BTreeMap<DataIndex, DataInfo>>, ManyError> {
        Ok(self
            .persistent_store
            .get(DATA_INFO_KEY)
            .map_err(error::storage_get_failed)?
            .map(|x| minicbor::decode(&x).unwrap()))
    }

    pub(crate) fn data_attributes(
        &self,
    ) -> Result<Option<BTreeMap<DataIndex, DataValue>>, ManyError> {
        Ok(self
            .persistent_store
            .get(DATA_ATTRIBUTES_KEY)
            .map_err(error::storage_get_failed)?
            .map(|x| minicbor::decode(&x).unwrap()))
    }

    pub(crate) fn update_account_count(
        &mut self,
        from: &Address,
        to: &Address,
        amount: TokenAmount,
        symbol: &Address,
    ) -> Result<(), ManyError> {
        if let Some(mut attributes) = self.data_attributes()? {
            let destination_key = key_for_account_balance(to, symbol);
            let destination_is_empty = self
                .persistent_store
                .get(&destination_key)
                .map_err(error::storage_get_failed)?
                .is_none();
            let destination_is_zero = self.get_balance(to, symbol)?.is_zero();

            // If the destination account does not exist, increase
            // account total count
            if destination_is_empty {
                attributes.entry(ACCOUNT_TOTAL_COUNT_INDEX).and_modify(|x| {
                    if let DataValue::Counter(count) = x {
                        *count += 1;
                    }
                });
            }
            // If the destination account either is empty or is zero,
            // the amount of non zero accounts increases
            if destination_is_zero || destination_is_empty {
                attributes
                    .entry(NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX)
                    .and_modify(|x| {
                        if let DataValue::Counter(count) = x {
                            *count += 1;
                        }
                    });
            }
            // If the amount from the origin account is equal to the
            // amount being sent, the account will become zero, hence
            // the non zero account total count decreases
            let origin_balance = self.get_balance(from, symbol)?;
            if origin_balance == amount {
                attributes
                    .entry(NON_ZERO_ACCOUNT_TOTAL_COUNT_INDEX)
                    .and_modify(|x| {
                        if let DataValue::Counter(count) = x {
                            *count -= 1;
                        }
                    });
            }
            self.persistent_store
                .apply(&[(
                    DATA_ATTRIBUTES_KEY.to_vec(),
                    Op::Put(minicbor::to_vec(attributes).unwrap()),
                )])
                .map_err(error::storage_apply_failed)?
        }
        Ok(())
    }
}
