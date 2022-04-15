use crate::protocol::{Attribute, AttributeId};
use crate::ManyError;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq)]
pub struct AttributeSet(BTreeSet<Attribute>);

impl AttributeSet {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert(&mut self, attr: Attribute) -> bool {
        self.0.insert(attr)
    }

    pub fn has_id(&self, id: AttributeId) -> bool {
        self.0.iter().any(|a| id == a.id)
    }

    pub fn contains(&self, a: &Attribute) -> bool {
        self.0.contains(a)
    }

    pub fn get_attribute(&self, id: AttributeId) -> Option<&Attribute> {
        self.0.iter().find(|a| id == a.id)
    }

    pub fn get<T: TryFromAttributeSet>(&self) -> Result<T, ManyError> {
        TryFromAttributeSet::try_from_set(self)
    }

    pub fn iter(&self) -> std::collections::btree_set::Iter<Attribute> {
        self.0.iter()
    }
}

pub trait TryFromAttributeSet: Sized {
    fn try_from_set(set: &AttributeSet) -> Result<Self, ManyError>;
}

impl IntoIterator for AttributeSet {
    type Item = Attribute;
    type IntoIter = std::collections::btree_set::IntoIter<Attribute>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromIterator<Attribute> for AttributeSet {
    fn from_iter<T: IntoIterator<Item = Attribute>>(iter: T) -> Self {
        Self(BTreeSet::from_iter(iter))
    }
}

impl Encode for AttributeSet {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        self.0.encode(e)
    }
}

impl<'b> Decode<'b> for AttributeSet {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        Ok(Self(BTreeSet::decode(d)?))
    }

    fn nil() -> Option<Self> {
        BTreeSet::nil().map(Self)
    }
}
