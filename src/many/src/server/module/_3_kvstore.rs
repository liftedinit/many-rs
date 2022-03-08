use crate::{Identity, ManyError};
use many_macros::many_module;

pub mod get;
pub mod info;
pub mod put;

pub use get::*;
pub use info::*;
pub use put::*;

#[many_module(name = KvStoreModule, id = 3, namespace = kvstore, many_crate = crate)]
pub trait KvStoreModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, ManyError>;
    fn get(&self, sender: &Identity, args: GetArgs) -> Result<GetReturns, ManyError>;
    fn put(&mut self, sender: &Identity, args: PutArgs) -> Result<PutReturns, ManyError>;
}
