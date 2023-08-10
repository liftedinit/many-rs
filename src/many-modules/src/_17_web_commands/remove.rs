use crate::EmptyReturn;
use many_identity::Address;
use many_types::Memo;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq)]
#[cbor(map)]
pub struct RemoveArgs {
    #[n(0)]
    pub owner: Option<Address>,

    #[n(1)]
    pub site_name: String,

    #[n(2)]
    pub memo: Option<Memo>,
}

pub type RemoveReturns = EmptyReturn;
