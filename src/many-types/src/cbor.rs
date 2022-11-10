use minicbor::data::{Tag, Type};
use minicbor::encode::Write;
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum CborAny {
    Undefined(),
    Bool(bool),
    Int(i64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<CborAny>),
    Map(BTreeMap<CborAny, CborAny>),
    Tagged(Tag, Box<CborAny>),
}

impl CborAny {
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Int(i) if i >= &0 => Some(*i as u64),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }
}

impl Debug for CborAny {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CborAny::Undefined() => write!(f, "undefined"),
            CborAny::Bool(b) => write!(f, "{b}"),
            CborAny::Int(i) => write!(f, "{i}"),
            CborAny::String(s) => f.write_str(s),
            CborAny::Bytes(b) => write!(f, r#"b"{}""#, hex::encode(b)),
            CborAny::Array(a) => write!(f, "{a:?}"),
            CborAny::Map(m) => write!(f, "{m:?}"),
            CborAny::Tagged(t, v) => write!(f, "tagged({t:?}, {v:?})"),
        }
    }
}

impl<C> Encode<C> for CborAny {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
            CborAny::Undefined() => {
                e.undefined()?;
            }
            CborAny::Bool(b) => {
                e.bool(*b)?;
            }
            CborAny::Int(i) => {
                e.i64(*i)?;
            }
            CborAny::String(s) => {
                e.str(s)?;
            }
            CborAny::Bytes(b) => {
                e.bytes(b)?;
            }
            CborAny::Array(arr) => {
                e.array(arr.len() as u64)?;
                for ref i in arr {
                    e.encode(i)?;
                }
            }
            CborAny::Map(m) => {
                e.encode(m)?;
            }
            CborAny::Tagged(t, v) => {
                e.tag(*t)?.encode(v)?;
            }
        }

        Ok(())
    }
}

impl<'d, C> Decode<'d, C> for CborAny {
    fn decode(d: &mut Decoder<'d>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        match d.datatype()? {
            Type::Bool => Ok(CborAny::Bool(d.bool()?)),
            Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::I8
            | Type::I16
            | Type::I32
            | Type::I64 => Ok(CborAny::Int(d.i64()?)),
            Type::Bytes => Ok(CborAny::Bytes(d.bytes()?.to_vec())),
            Type::String => Ok(CborAny::String(d.str()?.to_string())),
            Type::ArrayIndef | Type::Array => Ok(CborAny::Array(
                d.array_iter()?
                    .collect::<Result<Vec<CborAny>, minicbor::decode::Error>>()?,
            )),
            Type::MapIndef | Type::Map => {
                Ok(CborAny::Map(d.map_iter()?.collect::<Result<
                    BTreeMap<CborAny, CborAny>,
                    minicbor::decode::Error,
                >>()?))
            }
            Type::Tag => {
                let tag = d.tag()?;
                let value: CborAny = d.decode()?;
                Ok(CborAny::Tagged(tag, Box::new(value)))
            }
            x => Err(minicbor::decode::Error::type_mismatch(x)),
        }
    }
}

#[cfg(feature = "proptest")]
pub mod tests {
    use super::CborAny;
    use proptest::prelude::*;

    /// Generate arbitraty CborAny value.
    ///
    /// Recursive structures depth, size and branch size are limited
    pub fn arb_cbor() -> impl Strategy<Value = CborAny> {
        let leaf = prop_oneof![
            any::<bool>().prop_map(CborAny::Bool),
            any::<i64>().prop_map(CborAny::Int),
            ".*".prop_map(CborAny::String),
            proptest::collection::vec(any::<u8>(), 0..50).prop_map(CborAny::Bytes),
        ];

        leaf.prop_recursive(4, 256, 10, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..10).prop_map(CborAny::Array),
                proptest::collection::btree_map(inner.clone(), inner, 0..10).prop_map(CborAny::Map),
            ]
        })
    }
}
