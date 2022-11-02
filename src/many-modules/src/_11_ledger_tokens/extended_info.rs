use many_types::{AttributeRelatedIndex, Memo};
use std::collections::BTreeSet;
use visual_logo::VisualTokenLogo;

pub mod visual_logo;

#[derive()]
enum ExtendedInfo {
    Memo(Box<Memo>),
    VisualLogo(VisualTokenLogo),
}

impl PartialEq for ExtendedInfo {
    fn eq(&self, other: &Self) -> bool {
        todo!()
    }
}

pub struct TokenExtendedInfo {
    inner: BTreeSet<ExtendedInfo>,
}
