use async_trait::async_trait;
use many_error::ManyError;
use many_protocol::{RequestMessage, ResponseMessage};
use many_types::attributes::Attribute;
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
    ledger: _2_ledger + _6_ledger_commands + _11_ledger_tokens + _12_ledger_mintburn;
    events: _4_events;
    data: _5_data;
    kvstore: _3_kvstore + _7_kvstore_commands + _13_kvstore_transfer;
    r#async: _8_async;
    account: _9_account;
    abci_backend: _1000_abci_backend;
    abci_frontend: _1001_abci_frontend;
    idstore: _1002_idstore;
);

/// The specification says that some methods returns nothing (e.g. void or unit).
/// Empty returns are empty semantically (unit type), but we don't want to break CBOR
/// decoders so we use a null value instead.
/// We expect decoders to skip the value anyway.
#[derive(Debug, Eq, PartialEq)]
pub struct EmptyReturn;

impl<C> minicbor::Encode<C> for EmptyReturn {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        // We encode nothing as a null so it's a value.
        e.null()?;
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for EmptyReturn {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        // Nothing to do. Skip a value if there's one, but don't error if there's none.
        let _ = d.skip();
        Ok(Self)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct EmptyArg;

impl<C> minicbor::Encode<C> for EmptyArg {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        // We encode nothing as a null so it's a value.
        e.null()?;
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for EmptyArg {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        // Nothing to do. Skip a value if there's one, but don't error if there's none.
        let _ = d.skip();
        Ok(Self)
    }
}

#[derive(Clone, Debug)]
pub struct ManyModuleInfo {
    /// Returns the name of this module, for logs and metering.
    pub name: String,

    /// Returns the attribute supported by this module, if available.
    pub attribute: Option<Attribute>,

    /// The endpoints that this module exports.
    pub endpoints: Vec<String>,
}

/// A module ran by an many-server server.
#[async_trait]
pub trait ManyModule: Sync + Send + Debug {
    /// Returns information about this module.
    fn info(&self) -> &ManyModuleInfo;

    /// Verify that a message is well formed (ACLs, arguments, etc).
    /// This method has access to the envelope so it can validate COSE headers
    /// or other properties.
    fn validate(
        &self,
        _message: &RequestMessage,
        _envelope: &coset::CoseSign1,
    ) -> Result<(), ManyError> {
        Ok(())
    }

    /// Execute a message and returns its response.
    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError>;
}

#[cfg(test)]
pub(crate) mod testutils {
    use crate::ManyModule;
    use many_error::ManyError;
    use many_identity::testing::identity;
    use many_protocol::RequestMessage;

    pub fn call_module(
        key: u32,
        module: &'_ impl ManyModule,
        endpoint: impl ToString,
        payload: impl AsRef<str>,
    ) -> Result<Vec<u8>, ManyError> {
        call_module_cbor(
            key,
            module,
            endpoint,
            cbor_diag::parse_diag(payload).unwrap().to_bytes(),
        )
    }

    pub fn call_module_cbor(
        key: u32,
        module: &'_ impl ManyModule,
        endpoint: impl ToString,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, ManyError> {
        call_module_envelope(key, module, endpoint, payload, &coset::CoseSign1::default())
    }

    pub fn call_module_envelope(
        key: u32,
        module: &'_ impl ManyModule,
        endpoint: impl ToString,
        payload: Vec<u8>,
        envelope: &coset::CoseSign1,
    ) -> Result<Vec<u8>, ManyError> {
        let mut message = RequestMessage::default()
            .with_method(endpoint.to_string())
            .with_data(payload);

        message = if key > 0 {
            message.with_from(identity(key))
        } else {
            message
        };

        module.validate(&message, envelope)?;
        let response = smol::block_on(async { module.execute(message).await })?;
        response.data
    }
}
