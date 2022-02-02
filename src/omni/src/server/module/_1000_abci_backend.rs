use crate::OmniError;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};
use omni_module::omni_module;
use std::collections::BTreeMap;

#[derive(Debug, Encode, Decode)]
#[cbor(map)]
pub struct EndpointInfo {
    #[n(0)]
    pub should_commit: bool,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct AbciInit {
    /// List the methods supported by this module. For performance reason, this list will be
    /// cached and the only calls that will be sent to the backend module will be those
    /// listed in this list at initialization.
    /// This list is not private. If the OMNI Module needs to have some private endpoints,
    /// it should be implementing those separately. ABCI is not very compatible with private
    /// endpoints as it can't know if they change the state or not.
    #[n(0)]
    pub endpoints: BTreeMap<String, EndpointInfo>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct AbciInfo {
    #[n(0)]
    pub height: u64,

    #[n(1)]
    pub hash: ByteVec,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct AbciBlock {
    #[n(0)]
    pub time: Option<u64>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct AbciCommitInfo {
    #[n(0)]
    pub retain_height: u64,

    #[n(1)]
    pub hash: Vec<u8>,
}

/// A module that adapt an OMNI application to an ABCI-OMNI bridge.
/// This module takes a backend (another module) which ALSO implements the ModuleBackend
/// trait, and exposes the `abci.info` and `abci.init` endpoints.
/// This module should only be exposed to the tendermint server's network. It is not
/// considered secure (just like an ABCI app would not).
#[omni_module(name = AbciModule, id = 1000, namespace = abci, omni_crate = crate)]
pub trait OmniAbciModuleBackend: std::fmt::Debug + Send + Sync {
    /// Called when the ABCI frontend is initialized. No action should be taken here, only
    /// information should be returned. If the ABCI frontend is restarted, this method
    /// will be called again.
    fn init(&mut self) -> Result<AbciInit, OmniError>;

    /// Called at Genesis of the Tendermint blockchain.
    fn init_chain(&mut self) -> Result<(), OmniError>;

    /// Called at the start of a block.
    fn block_begin(&mut self, _info: AbciBlock) -> Result<(), OmniError> {
        Ok(())
    }

    /// Called when info is needed from the backend.
    fn info(&self) -> Result<AbciInfo, OmniError>;

    /// Called at the end of a block.
    fn block_end(&mut self) -> Result<(), OmniError> {
        Ok(())
    }

    /// Called after a block. The app should take this call and serialize its state.
    fn commit(&mut self) -> Result<AbciCommitInfo, OmniError>;
}
