use {
    super::{InnerStorage, Operation},
    crate::error,
    crate::storage::{key_for_account_balance, LedgerStorage},
    many_error::ManyError,
    many_identity::Address,
    many_modules::events::EventInfo,
    many_types::ledger::{Symbol, TokenAmount},
    many_types::Memo,
    std::cmp::Ordering,
    tracing::info,
};

impl LedgerStorage {
    pub fn get_balance(
        &self,
        identity: &Address,
        symbol: &Symbol,
    ) -> Result<TokenAmount, ManyError> {
        Ok(if identity.is_anonymous() {
            TokenAmount::zero()
        } else {
            let key = key_for_account_balance(identity, symbol);

            self.persistent_store
                .get(&key)
                .map_err(error::storage_get_failed)?
                .map_or(TokenAmount::zero(), TokenAmount::from)
        })
    }

    pub fn send(
        &mut self,
        from: &Address,
        to: &Address,
        symbol: &Symbol,
        amount: TokenAmount,
        memo: Option<Memo>,
    ) -> Result<impl IntoIterator<Item = Vec<u8>>, ManyError> {
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

        let batch: Vec<_> = match (&self.persistent_store, key_from.cmp(&key_to)) {
            (InnerStorage::V1(_), Ordering::Less) | (InnerStorage::V1(_), Ordering::Equal) => vec![
                (
                    key_from.clone(),
                    Operation::from(merk_v1::Op::Put(amount_from.to_vec())),
                ),
                (
                    key_to.clone(),
                    Operation::from(merk_v1::Op::Put(amount_to.to_vec())),
                ),
            ],
            (InnerStorage::V2(_), Ordering::Less) | (InnerStorage::V2(_), Ordering::Equal) => vec![
                (
                    key_from.clone(),
                    Operation::from(merk_v2::Op::Put(amount_from.to_vec())),
                ),
                (
                    key_to.clone(),
                    Operation::from(merk_v2::Op::Put(amount_to.to_vec())),
                ),
            ],
            (InnerStorage::V1(_), Ordering::Greater) => vec![
                (
                    key_to.clone(),
                    Operation::from(merk_v1::Op::Put(amount_to.to_vec())),
                ),
                (
                    key_from.clone(),
                    Operation::from(merk_v1::Op::Put(amount_from.to_vec())),
                ),
            ],
            (InnerStorage::V2(_), Ordering::Greater) => vec![
                (
                    key_to.clone(),
                    Operation::from(merk_v2::Op::Put(amount_to.to_vec())),
                ),
                (
                    key_from.clone(),
                    Operation::from(merk_v2::Op::Put(amount_from.to_vec())),
                ),
            ],
        };

        self.update_account_count(from, to, amount.clone(), symbol)?;

        self.persistent_store.apply(&batch)?;

        self.log_event(EventInfo::Send {
            from: *from,
            to: *to,
            symbol: *symbol,
            amount,
            memo,
        })?;

        self.maybe_commit().map(|_| vec![key_from, key_to])
    }
}
