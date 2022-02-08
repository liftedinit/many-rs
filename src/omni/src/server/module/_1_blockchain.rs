use crate::types::CborRange;
use crate::{define_attribute_omni_error, OmniError};
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};
use omni_module::omni_module;

define_attribute_omni_error!(
    attribute 1 => {
        1: pub fn height_out_of_bound(height, min, max)
            => "Height {height} is out of bound. Range: {min} - {max}.",
        2: pub fn invalid_hash() => "Requested hash does not have the right format.",
    }
);

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct InfoReturns {
    #[n(0)]
    pub hash: ByteVec,

    #[n(1)]
    pub app_hash: Option<ByteVec>,

    #[n(2)]
    pub height: u64,

    #[n(3)]
    pub retained_height: Option<u64>,
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
    pub messages: Vec<ByteVec>,
}

#[omni_module(name = BlockchainModule, id = 1, namespace = blockchain, omni_crate = crate)]
pub trait BlockchainModuleBackend: Send {
    fn info(&self) -> Result<InfoReturns, OmniError>;
    fn blocks(&self, args: BlocksArgs) -> Result<BlocksReturns, OmniError>;
}
