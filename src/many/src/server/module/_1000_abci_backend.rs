use crate::ManyError;
use many_macros::many_module;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Encode, Decode)]
#[cbor(map)]
pub struct EndpointInfo {
    #[n(0)]
    pub is_command: bool,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct AbciInit {
    /// List the methods supported by this module. For performance reason, this list will be
    /// cached and the only calls that will be sent to the backend module will be those
    /// listed in this list at initialization.
    /// This list is not private. If the MANY Module needs to have some private endpoints,
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
    pub hash: ByteVec,
}

#[derive(Encode, Decode, Debug)]
#[cbor(map)]
pub struct Snapshot {
    #[n(0)]
    pub path: PathBuf,
    #[n(1)]
    pub height: u64,
    #[n(2)]
    pub hash: Vec<u8>,
 //   #[n(3)]
 //   pub chunks: u64,
}

impl Snapshot {
    pub fn new(path: PathBuf, height: u64, hash: Vec<u8>) -> Self {
        Snapshot {
            path,
            height,
            hash,
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    pub fn hash(&self) -> &Vec<u8> {
        &self.hash
    }

    pub fn get_snap(&self) -> Result<Snapshot, ManyError> {
        Ok(Snapshot {
            path: self.path.clone(),
            height: self.height,
            hash: self.hash.clone(),
        })
    }
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct AbciListSnapshot {
    #[n(0)]
    pub all_snapshots: Vec<Snapshot>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct AbciOfferSnapshot {
    #[n(0)]
    pub snapshot: Option<Snapshot>,

    #[n(1)]
    pub app_hash: ByteVec,
}


/// A module that adapt a MANY application to an ABCI-MANY bridge.
/// This module takes a backend (another module) which ALSO implements the ModuleBackend
/// trait, and exposes the `abci.info` and `abci.init` endpoints.
/// This module should only be exposed to the tendermint server's network. It is not
/// considered secure (just like an ABCI app would not).
#[many_module(name = AbciModule, id = 1000, namespace = abci, many_crate = crate)]
pub trait ManyAbciModuleBackend: std::fmt::Debug + Send + Sync {
    /// Called when the ABCI frontend is initialized. No action should be taken here, only
    /// information should be returned. If the ABCI frontend is restarted, this method
    /// will be called again.
    fn init(&mut self) -> Result<AbciInit, ManyError>;

    /// Called at Genesis of the Tendermint blockchain.
    fn init_chain(&mut self) -> Result<(), ManyError>;

    /// Called at the start of a block.
    fn begin_block(&mut self, _info: AbciBlock) -> Result<(), ManyError> {
        Ok(())
    }

    /// Called when info is needed from the backend.
    fn info(&self) -> Result<AbciInfo, ManyError>;

    /// Called at the end of a block.
    fn end_block(&mut self) -> Result<(), ManyError> {
        Ok(())
    }

    /// Called after a block. The app should take this call and serialize its state.
    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError>;

    /// Called to list all available snapshots.
    fn list_snapshots(&mut self) -> Result<AbciListSnapshot, ManyError>;

    fn offer_snapshot(&mut self, _req: AbciOfferSnapshot) -> Result<(), ManyError>;
}
