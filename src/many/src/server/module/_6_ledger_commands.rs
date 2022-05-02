use crate::{Identity, ManyError};
use many_macros::many_module;

mod send;

pub use send::*;

#[many_module(name = LedgerCommandsModule, id = 6, namespace = ledger, many_crate = crate)]
pub trait LedgerCommandsModuleBackend: Send {
    fn send(&mut self, sender: &Identity, args: SendArgs) -> Result<(), ManyError>;
}
