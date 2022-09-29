use std::collections::BTreeMap;

use many_types::VecOrSingle;
use minicbor::{Encode, Decode};

use crate::data::{DataIndex, DataValue};

#[derive(Clone, Encode, Decode)]
pub struct DataQueryArgs {
    #[n(0)]
    pub indices: VecOrSingle<DataIndex>,
}

pub type DataQueryReturns = BTreeMap<DataIndex, DataValue>;
