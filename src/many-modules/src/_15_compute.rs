use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;

pub mod close;
pub mod deploy;
pub mod info;
pub mod list;

pub use close::*;
pub use deploy::*;
pub use info::*;
pub use list::*;

#[cfg(test)]
use mockall::{automock, predicate::*};

#[many_module(name = ComputeModule, id = 15, namespace = compute, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait ComputeModuleBackend: Send {
    fn info(&self, sender: &Address, args: InfoArg) -> Result<InfoReturns, ManyError>;

    #[many(deny_anonymous)]
    fn deploy(&mut self, sender: &Address, args: DeployArgs) -> Result<DeployReturns, ManyError>;

    #[many(deny_anonymous)]
    fn close(&mut self, sender: &Address, args: CloseArgs) -> Result<CloseReturns, ManyError>;

    fn list(&self, sender: &Address, args: ListArgs) -> Result<ListReturns, ManyError>;
}
