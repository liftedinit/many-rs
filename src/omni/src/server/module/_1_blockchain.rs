use crate::types::CborRange;
use crate::OmniError;
use minicbor::{Decode, Encode};
use omni_module::omni_module;

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct InfoReturns {
    #[n(0)]
    pub hash: Vec<u8>,

    #[n(1)]
    pub height: u64,

    #[n(2)]
    pub retained_height: u64,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BlocksArgs {
    #[n(0)]
    height: Option<CborRange<u64>>,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BlockHeader {
    #[n(0)]
    pub height: u64,
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct BlocksReturns {
    #[n(0)]
    pub header: BlockHeader,

    #[n(1)]
    pub messages: Vec<Vec<u8>>,
}

#[omni_module(name = BlockchainModule, id = 1, omni_crate = crate)]
pub trait BlockchainModuleBackend: Send {
    fn info(&self) -> Result<InfoReturns, OmniError>;
    fn blocks(&self, args: BlocksArgs) -> Result<BlocksReturns, OmniError>;
}
