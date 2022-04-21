use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Attribute;
use crate::ManyError;
use async_trait::async_trait;
use minicbor::encode::{Error, Write};
use minicbor::{Decoder, Encoder};
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

/// The specification says that some methods returns nothing (e.g. void or unit).
/// Empty returns are empty semantically (unit type), but we don't want to break CBOR
/// decoders so we use a null value instead.
/// We expect decoders to skip the value anyway.
#[derive(Debug)]
pub struct EmptyReturn;

impl minicbor::Encode for EmptyReturn {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        // We encode nothing as a null so it's a value.
        e.null()?;
        Ok(())
    }
}

impl<'b> minicbor::Decode<'b> for EmptyReturn {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        // Nothing to do. Skip a value if there's one, but don't error if there's none.
        let _ = d.skip();
        Ok(Self)
    }
}

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
