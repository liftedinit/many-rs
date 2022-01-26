use minicbor::data::Type;
use minicbor::encode::Write;
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum CborAny {
    Bool(bool),
    Int(i64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<CborAny>),
    Map(BTreeMap<CborAny, CborAny>),
}

impl Debug for CborAny {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CborAny::Bool(b) => write!(f, "{}", b),
            CborAny::Int(i) => write!(f, "{}", i),
            CborAny::String(s) => f.write_str(s),
            CborAny::Bytes(b) => write!(f, r#"b"{}""#, hex::encode(b)),
            CborAny::Array(a) => write!(f, "{:?}", a),
            CborAny::Map(m) => write!(f, "{:?}", m),
        }
    }
}

impl Encode for CborAny {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
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
                e.encode(&m)?;
            }
        }

        Ok(())
    }
}

impl<'d> Decode<'d> for CborAny {
    fn decode(d: &mut Decoder<'d>) -> Result<Self, minicbor::decode::Error> {
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
            x => Err(minicbor::decode::Error::TypeMismatch(
                x,
                "invalid attribute type.",
            )),
        }
    }
}
