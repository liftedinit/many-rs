use crate::protocol::{Attribute, AttributeId};
use crate::OmniError;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::BTreeSet;

pub mod response {
    use crate::cbor::CborAny;
    use crate::protocol::attributes::{AttributeSet, FromAttributeSet};
    use crate::protocol::Attribute;
    use crate::OmniError;

    pub const ASYNC: Attribute = Attribute::id(1);

    pub struct AsyncAttribute {
        pub token: Vec<u8>,
    }

    impl AsyncAttribute {
        pub fn new(token: Vec<u8>) -> Self {
            Self { token }
        }
    }

    impl From<AsyncAttribute> for Attribute {
        fn from(a: AsyncAttribute) -> Attribute {
            ASYNC.with_argument(CborAny::Bytes(a.token))
        }
    }

    impl TryFrom<Attribute> for AsyncAttribute {
        type Error = OmniError;

        fn try_from(value: Attribute) -> Result<Self, Self::Error> {
            if value.id != ASYNC.id {
                return Err(OmniError::invalid_attribute_id(value.id.to_string()));
            }

            let arguments = value.into_arguments();
            if arguments.len() != 1 {
                Err(OmniError::invalid_attribute_arguments())
            } else {
                match arguments.into_iter().next() {
                    Some(CborAny::Bytes(token)) => Ok(Self { token }),
                    _ => Err(OmniError::invalid_attribute_arguments()),
                }
            }
        }
    }

    impl FromAttributeSet for AsyncAttribute {
        fn from_set(set: &AttributeSet) -> Result<Self, OmniError> {
            match set.get_attribute(ASYNC.id) {
                Some(attr) => AsyncAttribute::try_from(attr.clone()),
                None => Err(OmniError::attribute_not_found(ASYNC.id.to_string())),
            }
        }
    }
}

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

    pub fn get<T: FromAttributeSet>(&self) -> Result<T, OmniError> {
        FromAttributeSet::from_set(self)
    }

    pub fn iter(&self) -> std::collections::btree_set::Iter<Attribute> {
        self.0.iter()
    }
}

pub trait FromAttributeSet: Sized {
    fn from_set(set: &AttributeSet) -> Result<Self, OmniError>;
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
