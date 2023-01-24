use crate::Setup;
use many_error::ManyError;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenInfoArgs, TokenInfoReturns};
use many_types::ledger::Symbol;

pub fn info(h: &Setup, symbol: Symbol) -> Result<TokenInfoReturns, ManyError> {
    LedgerTokensModuleBackend::info(
        &h.module_impl,
        &h.id,
        TokenInfoArgs {
            symbol,
            extended_info: None,
        },
    )
}
