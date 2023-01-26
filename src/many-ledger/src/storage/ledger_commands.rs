use crate::error;
use crate::storage::{key_for_account_balance, LedgerStorage};
use many_error::ManyError;
use many_identity::Address;
use many_modules::events::EventInfo;
use many_types::ledger::{Symbol, TokenAmount};
use many_types::Memo;
use merk::{BatchEntry, Op};
use std::cmp::Ordering;
use tracing::info;

impl LedgerStorage {
    pub fn get_balance(
        &self,
        identity: &Address,
        symbol: &Symbol,
    ) -> Result<TokenAmount, ManyError> {
        if identity.is_anonymous() {
            Ok(TokenAmount::zero())
        } else {
            let key = key_for_account_balance(identity, symbol);
            Ok(
                match self
                    .persistent_store
                    .get(&key)
                    .map_err(error::storage_get_failed)?
                {
                    None => TokenAmount::zero(),
                    Some(amount) => TokenAmount::from(amount),
                },
            )
        }
    }

    pub fn send(
        &mut self,
        from: &Address,
        to: &Address,
        symbol: &Symbol,
        amount: TokenAmount,
        memo: Option<Memo>,
    ) -> Result<(), ManyError> {
        if from == to {
            return Err(error::destination_is_source());
        }

        if amount.is_zero() {
            return Err(error::amount_is_zero());
        }

        if to.is_anonymous() || from.is_anonymous() {
            return Err(error::anonymous_cannot_hold_funds());
        }

        let mut amount_from = self.get_balance(from, symbol)?;
        if amount > amount_from {
            return Err(error::insufficient_funds());
        }

        info!("send({} => {}, {} {})", from, to, &amount, symbol);

        let mut amount_to = self.get_balance(to, symbol)?;
        amount_to += amount.clone();
        amount_from -= amount.clone();

        // Keys in batch must be sorted.
        let key_from = key_for_account_balance(from, symbol);
        let key_to = key_for_account_balance(to, symbol);

        let batch: Vec<BatchEntry> = match key_from.cmp(&key_to) {
            Ordering::Less | Ordering::Equal => vec![
                (key_from, Op::Put(amount_from.to_vec())),
                (key_to, Op::Put(amount_to.to_vec())),
            ],
            _ => vec![
                (key_to, Op::Put(amount_to.to_vec())),
                (key_from, Op::Put(amount_from.to_vec())),
            ],
        };

        self.update_account_count(from, to, amount.clone(), symbol)?;

        self.persistent_store
            .apply(&batch)
            .map_err(error::storage_apply_failed)?;

        self.log_event(EventInfo::Send {
            from: *from,
            to: *to,
            symbol: *symbol,
            amount,
            memo,
        })?;

        self.maybe_commit()?;

        Ok(())
    }
}
