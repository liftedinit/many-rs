use crate::error;
use crate::migration::tokens::TOKEN_MIGRATION;
use crate::module::LedgerModuleImpl;
use crate::storage::{account::verify_acl, SYMBOLS_ROOT};
use many_error::ManyError;
use many_identity::Address;
use many_modules::account::features::tokens::TokenAccountLedger;
use many_modules::account::features::TryCreateFeature;
use many_modules::account::Role;
use many_modules::ledger::{
    LedgerTokensModuleBackend, TokenAddExtendedInfoArgs, TokenAddExtendedInfoReturns,
    TokenCreateArgs, TokenCreateReturns, TokenInfoArgs, TokenInfoReturns,
    TokenRemoveExtendedInfoArgs, TokenRemoveExtendedInfoReturns, TokenUpdateArgs,
    TokenUpdateReturns,
};
use many_protocol::context::Context;
use many_types::Either;

fn check_ticker_length(ticker: &String) -> Result<(), ManyError> {
    if !(3..=5).contains(&ticker.len()) {
        return Err(error::invalid_ticker_length(ticker));
    }
    Ok(())
}

impl LedgerTokensModuleBackend for LedgerModuleImpl {
    fn create(
        &mut self,
        sender: &Address,
        args: TokenCreateArgs,
        context: Context,
    ) -> Result<TokenCreateReturns, ManyError> {
        #[cfg(not(feature = "disable_token_sender_check"))]
        use crate::storage::{ledger_tokens::TOKEN_IDENTITY_ROOT, IDENTITY_ROOT};
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.create"));
        }

        let mut keys = vec![SYMBOLS_ROOT.to_string().into_bytes()];

        #[cfg(not(feature = "disable_token_sender_check"))]
        crate::storage::ledger_tokens::verify_tokens_sender(
            sender,
            self.storage
                .get_identity(TOKEN_IDENTITY_ROOT)
                .map(|identity| {
                    keys.push(TOKEN_IDENTITY_ROOT.into());
                    identity
                })
                .or_else(|_| {
                    self.storage.get_identity(IDENTITY_ROOT).map(|identity| {
                        keys.push(IDENTITY_ROOT.into());
                        identity
                    })
                })?,
        )?;

        if let Some(Either::Left(addr)) = &args.owner {
            keys.extend(verify_acl(
                &self.storage,
                sender,
                addr,
                [Role::CanTokensCreate],
                TokenAccountLedger::ID,
            )?);
        }

        let ticker = &args.summary.ticker;
        check_ticker_length(ticker)?;

        if self
            .storage
            .get_symbols_and_tickers()?
            .values()
            .any(|v| v == ticker)
        {
            return Err(ManyError::unknown(format!(
                "The ticker {ticker} already exists on this network"
            )));
        }
        let (result, token_creation_keys) = self.storage.create_token(sender, args)?;
        keys.extend(token_creation_keys);
        self.storage
            .prove_state(context, keys)
            .map(|error| ManyError::unknown(error.to_string()))
            .map(Err)
            .unwrap_or(Ok(()))
            .map(|_| result)
    }

    fn info(&self, _sender: &Address, args: TokenInfoArgs) -> Result<TokenInfoReturns, ManyError> {
        // Check the memory symbol cache for requested symbol
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.info"));
        }

        let symbol = &args.symbol;
        if !self.storage.get_symbols()?.contains(symbol) {
            return Err(ManyError::unknown(format!(
                "The symbol {symbol} was not found"
            )));
        }
        self.storage.info_token(args)
    }

    fn update(
        &mut self,
        sender: &Address,
        args: TokenUpdateArgs,
        context: Context,
    ) -> Result<TokenUpdateReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.update"));
        }

        let mut keys: Vec<Vec<u8>> = vec![SYMBOLS_ROOT.into()];

        // Get the current owner and check if we're allowed to update this token
        let (current_owner, owner_key) = self.storage.get_owner(&args.symbol)?;
        keys.push(owner_key);
        match current_owner {
            Some(addr) => {
                let _ = verify_acl(
                    &self.storage,
                    sender,
                    &addr,
                    [Role::CanTokensUpdate],
                    TokenAccountLedger::ID,
                )?;
            }
            None => {
                return Err(ManyError::unknown(
                    "Unable to update, this token is immutable",
                ))
            }
        }

        // Check the memory symbol cache for requested symbol
        let symbol = &args.symbol;
        if !self.storage.get_symbols()?.contains(symbol) {
            return Err(ManyError::unknown(format!(
                "The symbol {symbol} was not found"
            )));
        }

        if let Some(ticker) = &args.ticker {
            check_ticker_length(ticker)?;
        }

        let (result, update_keys) = self.storage.update_token(sender, args)?;
        keys.extend(update_keys);
        self.storage
            .prove_state(context, keys)
            .map(|error| ManyError::unknown(error.to_string()))
            .map(Err)
            .unwrap_or(Ok(()))
            .map(|_| result)
    }

    fn add_extended_info(
        &mut self,
        sender: &Address,
        args: TokenAddExtendedInfoArgs,
        _: Context
    ) -> Result<TokenAddExtendedInfoReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.addExtendedInfo"));
        }

        let (current_owner, _) = self.storage.get_owner(&args.symbol)?;
        match current_owner {
            Some(addr) => {
                let _ = verify_acl(
                    &self.storage,
                    sender,
                    &addr,
                    [Role::CanTokensAddExtendedInfo],
                    TokenAccountLedger::ID,
                )?;
            }
            None => {
                return Err(ManyError::unknown(
                    "Unable to update, this token is immutable",
                ))
            }
        }

        let (result, _) = self.storage.add_extended_info(args)?;
        Ok(result)
    }

    fn remove_extended_info(
        &mut self,
        sender: &Address,
        args: TokenRemoveExtendedInfoArgs,
        _: Context
    ) -> Result<TokenRemoveExtendedInfoReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.removeExtendedInfo"));
        }

        let (current_owner, _) = self.storage.get_owner(&args.symbol)?;
        match current_owner {
            Some(addr) => {
                let _ = verify_acl(
                    &self.storage,
                    sender,
                    &addr,
                    [Role::CanTokensRemoveExtendedInfo],
                    TokenAccountLedger::ID,
                )?;
            }
            None => {
                return Err(ManyError::unknown(
                    "Unable to update, this token is immutable",
                ))
            }
        }

        let (result, _) = self.storage.remove_extended_info(args)?;
        Ok(result)
    }
}
