use crate::account::features::multisig::Memo;
use many_types::AttributeRelatedIndex;
use std::collections::BTreeSet;

pub enum ExtendedInfo {
    memo(Box<Memo<4000>>),
}

pub struct TokenExtendedInfo {
    inner: BTreeSet<ExtendedInfo>,
}
