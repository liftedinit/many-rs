use crate::{EmptyArg, data::DataIndex};
use minicbor::{Decode, Encode};

pub type DataInfoArgs = EmptyArg;

#[derive(Clone, Decode, Encode)]
pub struct DataInfoReturns {
    #[n(0)]
    pub indices: Vec<DataIndex>
}
