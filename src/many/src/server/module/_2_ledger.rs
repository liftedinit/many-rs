use crate::{define_attribute_many_error, Identity, ManyError};
use many_macros::many_module;

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

define_attribute_many_error!(
    attribute 2 => {
        1: pub fn unknown_symbol(symbol) => "Symbol not supported by this ledger: {symbol}.",
        2: pub fn unauthorized() => "Unauthorized to do this operation.",
        3: pub fn insufficient_funds() => "Insufficient funds.",
        4: pub fn anonymous_cannot_hold_funds() => "Anonymous is not a valid account identity.",
        5: pub fn invalid_initial_state(expected, actual)
            => "Invalid initial state hash. Expected '{expected}', was '{actual}'.",
    }
);

#[many_module(name = LedgerModule, id = 2, namespace = ledger, many_crate = crate)]
pub trait LedgerModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, ManyError>;
    fn balance(&self, sender: &Identity, args: BalanceArgs) -> Result<BalanceReturns, ManyError>;
    fn mint(&mut self, sender: &Identity, args: MintArgs) -> Result<(), ManyError>;
    fn burn(&mut self, sender: &Identity, args: BurnArgs) -> Result<(), ManyError>;
    fn send(&mut self, sender: &Identity, args: SendArgs) -> Result<(), ManyError>;
}
