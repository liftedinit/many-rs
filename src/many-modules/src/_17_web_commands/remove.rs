use crate::EmptyReturn;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
#[cbor(map)]
pub struct RemoveArgs {
    #[n(0)]
    pub site_name: String,
}

pub type RemoveReturns = EmptyReturn;
