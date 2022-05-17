use crate::ManyError;
use many_macros::many_module;

mod store;
mod get;
pub mod errors;
pub mod types;

pub use store::*;
pub use get::*;
pub use errors::*;
pub use types::*;

#[many_module(name = IdStoreModule, id = 1002, namespace = idstore, many_crate = crate)]
pub trait IdStoreModuleBackend: Send {
    fn store(&mut self, args: StoreArgs) -> Result<StoreReturns, ManyError>;
    fn get_from_recall_phrase(&self, args: GetFromRecallPhraseArgs) -> Result<GetReturns, ManyError>;
    fn get_from_address(&self, args: GetFromAddressArgs) -> Result<GetReturns, ManyError>;
}

