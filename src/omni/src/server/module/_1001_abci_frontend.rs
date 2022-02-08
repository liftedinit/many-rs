use crate::{define_attribute_omni_error, OmniError};
use minicbor::{Decode, Encode};
use omni_module::omni_module;

define_attribute_omni_error!(
    attribute 1001 => {
        1: pub fn abci_transport_error(details) => "ABCI interface returned an error: {details}.",
    }
);

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct StatusReturns {}

#[omni_module(name = AbciFrontendModule, id = 1001, namespace = abci, omni_crate = crate)]
pub trait AbciClientModuleBackend: Send {
    fn status(&self) -> Result<StatusReturns, OmniError>;
}
