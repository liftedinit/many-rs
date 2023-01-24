use crate::error;
use crate::migration::tokens::TOKEN_MIGRATION;
use crate::module::LedgerModuleImpl;
use crate::storage::ledger_tokens::verify_tokens_sender;
use many_error::ManyError;
use many_identity::Address;
use many_modules::events::EventInfo;
use many_modules::ledger;
use many_modules::ledger::{TokenBurnArgs, TokenBurnReturns, TokenMintArgs, TokenMintReturns};
use many_types::ledger::Symbol;
use std::collections::BTreeSet;

/// Check if a symbol exists in the storage
fn check_symbol_exists(symbol: &Symbol, symbols: BTreeSet<Symbol>) -> Result<(), ManyError> {
    if !symbols.contains(symbol) {
        return Err(error::symbol_not_found(symbol.to_string()));
    }
    Ok(())
}

impl ledger::LedgerMintBurnModuleBackend for LedgerModuleImpl {
    fn mint(
        &mut self,
        sender: &Address,
        args: TokenMintArgs,
    ) -> Result<TokenMintReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.mint"));
        }

        let TokenMintArgs {
            symbol,
            distribution,
            memo,
        } = args;
        // Only the token identity is able to mint tokens
        verify_tokens_sender(
            sender,
            self.storage
                .get_identity(crate::storage::ledger_tokens::TOKEN_IDENTITY_ROOT)
                .or_else(|_| self.storage.get_identity(crate::storage::IDENTITY_ROOT))?,
        )?;

        check_symbol_exists(&symbol, self.storage.get_symbols()?)?;

        // Mint into storage
        self.storage.mint_token(symbol, &distribution)?;

        // Log event
        self.storage.log_event(EventInfo::TokenMint {
            symbol,
            distribution,
            memo,
        })?;

        Ok(TokenMintReturns {})
    }

    fn burn(
        &mut self,
        sender: &Address,
        args: TokenBurnArgs,
    ) -> Result<TokenBurnReturns, ManyError> {
        if !self.storage.migrations().is_active(&TOKEN_MIGRATION) {
            return Err(ManyError::invalid_method_name("tokens.burn"));
        }

        let TokenBurnArgs {
            symbol,
            distribution,
            memo,
            error_on_under_burn,
        } = args;
        // Only the token identity is able to burn tokens
        verify_tokens_sender(
            sender,
            self.storage
                .get_identity(crate::storage::ledger_tokens::TOKEN_IDENTITY_ROOT)
                .or_else(|_| self.storage.get_identity(crate::storage::IDENTITY_ROOT))?,
        )?;

        check_symbol_exists(&symbol, self.storage.get_symbols()?)?;

        // Disable partial burn, for now
        if let Some(error) = error_on_under_burn {
            if !error {
                return Err(error::partial_burn_disabled());
            }
        }

        // Burn from storage
        self.storage.burn_token(symbol, &distribution)?;

        // Log event
        self.storage.log_event(EventInfo::TokenBurn {
            symbol,
            distribution: distribution.clone(),
            memo,
        })?;

        Ok(TokenBurnReturns { distribution })
    }
}
