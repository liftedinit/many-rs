use many_error::{define_attribute_many_error, ManyError};
use many_macros::many_module;
use minicbor::{Decode, Encode};

define_attribute_many_error!(
    attribute 1001 => {
        1: pub fn abci_transport_error(details) => "ABCI interface returned an error: {details}.",
    }
);

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct StatusReturns {}

#[many_module(name = AbciFrontendModule, id = 1001, namespace = abci, many_modules_crate = crate)]
pub trait AbciClientModuleBackend: Send {
    fn status(&self) -> Result<StatusReturns, ManyError>;
}
