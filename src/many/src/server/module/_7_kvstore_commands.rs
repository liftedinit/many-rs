use crate::{Identity, ManyError};
use many_macros::many_module;

mod delete;
mod put;
pub use delete::*;
pub use put::*;

#[many_module(name = KvStoreCommandsModule, id = 7, namespace = kvstore, many_crate = crate)]
pub trait KvStoreCommandsModuleBackend: Send {
    fn put(&mut self, sender: &Identity, args: PutArgs) -> Result<PutReturns, ManyError>;
    fn delete(&mut self, sender: &Identity, args: DeleteArgs) -> Result<DeleteReturn, ManyError>;
}
