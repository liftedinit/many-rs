use crate::{Identity, ManyError};
use many_macros::many_module;

mod put;
pub use put::*;

#[many_module(name = KvStoreCommandsModule, id = 5, namespace = kvstore, many_crate = crate)]
pub trait KvStoreCommandsModuleBackend: Send {
    fn put(&mut self, sender: &Identity, args: PutArgs) -> Result<PutReturns, ManyError>;
}
