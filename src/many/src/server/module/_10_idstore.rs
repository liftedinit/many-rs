use crate::ManyError;
use many_macros::many_module;

mod store;
mod get;

pub use store::*;
pub use get::*;

#[many_module(name = IdStoreModule, id = 4, namespace = idstore, many_crate = crate)]
pub trait IdStoreModuleBackend: Send {
    fn store(&self, args: StoreArgs) -> Result<StoreReturn, ManyError>;
    fn get_from_recall_phrase(&self, args: GetFromRecallPhraseArgs) -> Result<GetReturns, ManyError>;
    fn get_from_address(&self, args: GetFromAddressArgs) -> Result<GetReturns, ManyError>;
}
