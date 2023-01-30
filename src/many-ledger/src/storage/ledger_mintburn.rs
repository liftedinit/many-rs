use crate::error;
use crate::storage::ledger_tokens::key_for_symbol;
use crate::storage::{key_for_account_balance, LedgerStorage};
use many_error::ManyError;
use many_modules::ledger::TokenInfoArgs;
use many_types::ledger::{LedgerTokensAddressMap, Symbol, TokenAmount, TokenInfoSupply};
use merk::{BatchEntry, Op};
use std::collections::BTreeSet;

impl LedgerStorage {
    pub(crate) fn get_token_supply(&self, symbol: &Symbol) -> Result<TokenInfoSupply, ManyError> {
        Ok(self
            .info_token(TokenInfoArgs {
                symbol: *symbol,
                extended_info: None,
            })?
            .info
            .supply)
    }

    pub fn mint_token(
        &mut self,
        symbol: Symbol,
        distribution: &LedgerTokensAddressMap,
    ) -> Result<(), ManyError> {
        let mut batch: Vec<BatchEntry> = Vec::new();
        let mut circulating = TokenAmount::zero();
        let current_supply = self.get_token_supply(&symbol)?;

        for (address, amount) in distribution.iter() {
            if amount.is_zero() {
                return Err(error::unable_to_distribute_zero(address));
            }

            circulating += amount;

            // Make sure we don't bust the maximum, if any
            match &current_supply.maximum {
                Some(x) if &(&current_supply.circulating + &circulating) > x => {
                    return Err(error::over_maximum_supply(symbol, circulating, x))
                }
                _ => {}
            }

            // Store the new balance to the DB
            let new_balance = self
                .get_multiple_balances(address, &BTreeSet::from([symbol]))?
                .get(&symbol)
                .map_or(amount.clone(), |b| b + amount);
            let key = key_for_account_balance(address, &symbol);
            batch.push((key, Op::Put(new_balance.to_vec())));
        }

        // Update circulating supply
        let mut info = self
            .info_token(TokenInfoArgs {
                symbol,
                extended_info: None,
            })?
            .info;
        info.supply.circulating += &circulating;
        info.supply.total += circulating;

        batch.push((
            key_for_symbol(&symbol).into(),
            Op::Put(minicbor::to_vec(&info).map_err(ManyError::serialization_error)?),
        ));

        self.persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        self.maybe_commit()?;

        Ok(())
    }

    pub fn burn_token(
        &mut self,
        symbol: Symbol,
        distribution: &LedgerTokensAddressMap,
    ) -> Result<(), ManyError> {
        let mut batch: Vec<BatchEntry> = Vec::new();
        let mut circulating = TokenAmount::zero();

        for (address, amount) in distribution.iter() {
            if amount.is_zero() {
                return Err(error::unable_to_distribute_zero(address));
            }

            // Check if we have enough funds
            let balance_amount = match self
                .get_multiple_balances(address, &BTreeSet::from_iter([symbol]))?
                .get(&symbol)
            {
                Some(x) if x < amount => Err(error::missing_funds(symbol, amount, x)),
                Some(x) => Ok(x.clone()),
                None => Err(error::missing_funds(symbol, amount, TokenAmount::zero())),
            }?;

            // Store new balance in DB
            let new_balance = &balance_amount - amount;
            let key = key_for_account_balance(address, &symbol);
            batch.push((key, Op::Put(new_balance.to_vec())));
            circulating += amount;
        }

        // Update circulating supply
        let mut info = self
            .info_token(TokenInfoArgs {
                symbol,
                extended_info: None,
            })?
            .info;
        info.supply.circulating -= &circulating;
        info.supply.total -= circulating;

        batch.push((
            key_for_symbol(&symbol).into(),
            Op::Put(minicbor::to_vec(&info).map_err(ManyError::serialization_error)?),
        ));

        self.persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        self.maybe_commit()?;

        Ok(())
    }
}
