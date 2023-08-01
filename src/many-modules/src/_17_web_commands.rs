use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;

pub mod deploy;
pub mod remove;

pub use deploy::*;
pub use remove::*;

#[cfg(test)]
use mockall::{automock, predicate::*};

#[many_module(name = WebCommandsModule, id = 17, namespace = web, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait WebCommandsModuleBackend: Send {
    #[many(deny_anonymous)]
    fn deploy(&mut self, sender: &Address, args: DeployArgs) -> Result<DeployReturns, ManyError>;

    #[many(deny_anonymous)]
    fn remove(&mut self, sender: &Address, args: RemoveArgs) -> Result<RemoveReturns, ManyError>;
}
