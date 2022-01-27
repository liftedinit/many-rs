use crate::{Identity, OmniError};
use omni_module::omni_module;

pub mod get;
pub mod info;
pub mod put;

pub use get::*;
pub use info::*;
pub use put::*;

#[omni_module(name = KvStoreModule, id = 3, namespace = kvstore, omni_crate = crate)]
pub trait KvStoreModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, OmniError>;
    fn get(&self, sender: &Identity, args: GetArgs) -> Result<GetReturns, OmniError>;
    fn put(&mut self, sender: &Identity, args: PutArgs) -> Result<PutReturns, OmniError>;
}
