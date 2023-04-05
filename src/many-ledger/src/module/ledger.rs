use crate::{module::LedgerModuleImpl, storage::SYMBOLS_ROOT};
use many_error::ManyError;
use many_identity::Address;
use many_modules::ledger;
use many_protocol::context::Context;
use std::collections::BTreeSet;
use tracing::info;

impl ledger::LedgerModuleBackend for LedgerModuleImpl {
    fn info(
        &self,
        _: &Address,
        _: ledger::InfoArgs,
        context: Context,
    ) -> Result<ledger::InfoReturns, ManyError> {
        let storage = &self.storage;

        // Hash the storage.
        let hash = storage.hash();
        println!("Hash: {hash:?}");
        let symbols = storage.get_symbols_and_tickers()?;
        println!("Symbols and tickers:");
        println!("{symbols:?}");

        storage.prove_state(
            context,
            vec![hash.clone(), SYMBOLS_ROOT.as_bytes().to_vec()],
        )?;
        println!("Storage proved");

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
        ledger::BalanceArgs { account, symbols }: ledger::BalanceArgs,
        context: Context,
    ) -> Result<ledger::BalanceReturns, ManyError> {
        let identity = account.as_ref().unwrap_or(sender);

        let storage = &self.storage;
        let symbols = symbols.unwrap_or_default().0;

        let (balances, keys) = storage
            .get_multiple_balances(identity, &BTreeSet::from_iter(symbols.clone().into_iter()))?;
        storage.prove_state(context, keys)?;
        info!("balance({}, {:?}): {:?}", identity, &symbols, &balances);
        Ok(ledger::BalanceReturns { balances })
    }
}
