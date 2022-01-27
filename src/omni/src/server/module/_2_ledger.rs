use crate::{Identity, OmniError};
use omni_module::omni_module;

mod balance;
mod burn;
mod info;
mod mint;
mod send;

pub use balance::*;
pub use burn::*;
pub use info::*;
pub use mint::*;
pub use send::*;

#[omni_module(name = LedgerModule, id = 2, namespace = ledger, omni_crate = crate)]
pub trait LedgerModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, OmniError>;
    fn balance(&self, sender: &Identity, args: BalanceArgs) -> Result<BalanceReturns, OmniError>;
    fn mint(&mut self, sender: &Identity, args: MintArgs) -> Result<(), OmniError>;
    fn burn(&mut self, sender: &Identity, args: BurnArgs) -> Result<(), OmniError>;
    fn send(&mut self, sender: &Identity, args: SendArgs) -> Result<(), OmniError>;
}
