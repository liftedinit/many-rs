use crate::server::module::EmptyArg;
use minicbor::{Decode, Encode};

pub type InfoArgs = EmptyArg;

#[derive(Decode, Encode)]
#[cbor(map)]
pub struct InfoReturn {
    #[n(0)]
    pub total: u64,
}
