use minicbor::{Decode, Encode};
use crate::EmptyReturn;

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct RemoveArgs {
    #[n(0)]
    pub site_name: String,
}

pub type RemoveReturns = EmptyReturn;