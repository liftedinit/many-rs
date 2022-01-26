use crate as omni;
use crate::OmniError;
use omni_module::omni_module;

mod info;
mod list;

pub use info::*;
pub use list::*;

#[omni_module(name = LedgerTransactionsModule, id = 4, namespace = ledger)]
pub trait LedgerTransactionsModuleBackend: Send {
    fn transactions(&self, args: TransactionsArgs) -> Result<TransactionsReturns, OmniError>;
    fn list(&mut self, args: ListArgs) -> Result<ListReturns, OmniError>;
}
