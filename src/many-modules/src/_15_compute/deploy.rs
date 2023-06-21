use many_types::compute::{ByteUnits, ComputeStatus, ProviderInfo, Region};
use minicbor::{Decode, Encode};

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct DeployArgs {
    #[n(0)]
    pub image: String,
    #[n(1)]
    pub port: u16,
    #[n(2)]
    pub num_cpu: f64,
    #[n(3)]
    pub num_memory: u64,
    #[n(4)]
    pub memory_type: ByteUnits,
    #[n(5)]
    pub num_storage: u64,
    #[n(6)]
    pub storage_type: ByteUnits,
    #[n(7)]
    pub region: Region,
}

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct DeployReturns {
    #[n(0)]
    pub status: ComputeStatus,
    #[n(1)]
    pub provider_info: ProviderInfo,
    #[n(2)]
    pub dseq: u64,
}
