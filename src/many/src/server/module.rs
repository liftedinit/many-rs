use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Attribute;
use crate::ManyError;
use async_trait::async_trait;
use std::fmt::Debug;

macro_rules! reexport_module {
    ( $( $rename: ident: $name: ident $(+ $more: ident)*; )* ) => {
        $(
            mod $name;
            $(mod $more;)*

            pub mod $rename {
                pub use super::$name::*;
                $(pub use super::$more::*;)*
            }
        )*
    };
}

reexport_module!(
    base: _0_base;
    blockchain: _1_blockchain;
    ledger: _2_ledger + _4_ledger_transactions + _6_ledger_commands;
    kvstore: _3_kvstore + _7_kvstore_commands;
    r#async: _8_async;
    abci_backend: _1000_abci_backend;
    abci_frontend: _1001_abci_frontend;
);

#[derive(Clone, Debug)]
pub struct ManyModuleInfo {
    /// Returns the name of this module, for logs and metering.
    pub name: String,

    /// Returns a list of all attributes supported by this module.
    pub attribute: Attribute,

    /// The endpoints that this module exports.
    pub endpoints: Vec<String>,
}

/// A module ran by an many server.
#[async_trait]
pub trait ManyModule: Sync + Send + Debug {
    /// Returns information about this module.
    fn info(&self) -> &ManyModuleInfo;

    /// Verify that a message is well formed (ACLs, arguments, etc).
    fn validate(&self, _message: &RequestMessage) -> Result<(), ManyError> {
        Ok(())
    }

    /// Execute a message and returns its response.
    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError>;
}
