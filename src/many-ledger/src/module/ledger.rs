use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::ledger;
use std::collections::BTreeSet;
use tracing::info;

impl ledger::LedgerModuleBackend for LedgerModuleImpl {
    fn info(
        &self,
        _sender: &Address,
        _args: ledger::InfoArgs,
    ) -> Result<ledger::InfoReturns, ManyError> {
        let storage = &self.storage;

        // Hash the storage.
        let hash = storage.hash();
        let symbols = storage.get_symbols_and_tickers()?;

        info!(
            "info(): hash={} symbols={:?}",
            hex::encode(storage.hash()).as_str(),
            symbols
        );

        Ok(ledger::InfoReturns {
            symbols: symbols.keys().copied().collect(),
            hash: hash.into(),
            local_names: symbols,
            tokens: storage.get_token_info_summary()?,
        })
    }

    fn balance(
        &self,
        sender: &Address,
        args: ledger::BalanceArgs,
    ) -> Result<ledger::BalanceReturns, ManyError> {
        let ledger::BalanceArgs { account, symbols } = args;

        let identity = account.as_ref().unwrap_or(sender);

        let storage = &self.storage;
        let symbols = symbols.unwrap_or_default().0;

        let balances = storage
            .get_multiple_balances(identity, &BTreeSet::from_iter(symbols.clone().into_iter()))?;
        info!("balance({}, {:?}): {:?}", identity, &symbols, &balances);
        Ok(ledger::BalanceReturns { balances })
    }
}
