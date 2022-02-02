use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Attribute;
use crate::OmniError;
use async_trait::async_trait;
use std::fmt::Debug;

macro_rules! reexport_module {
    ( $( $name: ident = $rename: ident; )* ) => {
        $(
            mod $name;
            pub mod $rename {
                pub use super::$name::*;
            }
        )*
    };
}

reexport_module!(
    _0_base = base;
    _1_blockchain = blockchain;
    _2_ledger = ledger;
    _3_kvstore = kvstore;
    _4_ledger_transactions = ledger_transactions;
    _1000_abci_backend = abci_backend;
);

#[derive(Clone, Debug)]
pub struct OmniModuleInfo {
    /// Returns the name of this module, for logs and metering.
    pub name: String,

    /// Returns a list of all attributes supported by this module.
    pub attribute: Attribute,

    /// The endpoints that this module exports.
    pub endpoints: Vec<String>,
}

/// A module ran by an omni server.
#[async_trait]
pub trait OmniModule: Sync + Send + Debug {
    /// Returns information about this module.
    fn info(&self) -> &OmniModuleInfo;

    /// Verify that a message is well formed (ACLs, arguments, etc).
    fn validate(&self, _message: &RequestMessage) -> Result<(), OmniError> {
        Ok(())
    }

    /// Execute a message and returns its response.
    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError>;
}
