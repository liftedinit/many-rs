use crate::{Identity, ManyError};
use many_macros::many_module;

mod burn;
mod mint;
mod send;

pub use burn::*;
pub use mint::*;
pub use send::*;

#[many_module(name = LedgerCommandsModule, id = 6, namespace = ledger, many_crate = crate)]
pub trait LedgerCommandsModuleBackend: Send {
    fn mint(&mut self, sender: &Identity, args: MintArgs) -> Result<(), ManyError>;
    fn burn(&mut self, sender: &Identity, args: BurnArgs) -> Result<(), ManyError>;
    fn send(&mut self, sender: &Identity, args: SendArgs) -> Result<(), ManyError>;
}
