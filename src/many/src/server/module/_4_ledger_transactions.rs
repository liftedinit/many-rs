use crate::ManyError;
use many_macros::many_module;

mod info;
mod list;

pub use info::*;
pub use list::*;

#[many_module(name = LedgerTransactionsModule, id = 4, namespace = ledger, many_crate = crate)]
pub trait LedgerTransactionsModuleBackend: Send {
    fn transactions(&self, args: TransactionsArgs) -> Result<TransactionsReturns, ManyError>;
    fn list(&mut self, args: ListArgs) -> Result<ListReturns, ManyError>;
}
