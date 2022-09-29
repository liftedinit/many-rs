use std::collections::BTreeMap;

use many_types::VecOrSingle;
use minicbor::{Encode, Decode};

use crate::data::{DataIndex, DataInfo};

#[derive(Clone, Encode, Decode)]
pub struct DataGetInfoArgs {
    #[n(0)]
    pub indices: VecOrSingle<DataIndex>
}

pub type DataGetInfoReturns = BTreeMap<DataIndex, DataInfo>;
