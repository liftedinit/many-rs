use minicbor::data::{Tag, Type};
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CborNull;

impl<C> Encode<C> for CborNull {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.null()?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for CborNull {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        d.null()?;
        Ok(CborNull)
    }
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum CborAny {
    Bool(bool),
    Int(i64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<CborAny>),
    Map(BTreeMap<CborAny, CborAny>),
    Tagged(Tag, Box<CborAny>),
    Null,
}

impl Debug for CborAny {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CborAny::Bool(b) => write!(f, "{b}"),
            CborAny::Int(i) => write!(f, "{i}"),
            CborAny::String(s) => f.write_str(s),
            CborAny::Bytes(b) => write!(f, r#"b"{}""#, hex::encode(b)),
            CborAny::Array(a) => write!(f, "{a:?}"),
            CborAny::Map(m) => write!(f, "{m:?}"),
            CborAny::Tagged(t, v) => write!(f, "{t:?}({v:?})"),
            CborAny::Null => write!(f, "Null"),
        }
    }
}

impl<C> Encode<C> for CborAny {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
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
                e.encode(m)?;
            }
            CborAny::Tagged(t, v) => {
                e.tag(*t)?.encode(v)?;
            }
            CborAny::Null => {
                e.null()?;
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
            Type::Tag => Ok(CborAny::Tagged(d.tag()?, Box::new(d.decode()?))),
            Type::Null => {
                d.skip()?;
                Ok(CborAny::Null)
            }
            x => Err(minicbor::decode::Error::type_mismatch(x)),
        }
    }
}

/// Encode/Decode cbor in a Base64 String instead of its CBOR value. `T` must be
/// transformable to (Deref) and from (FromIterator<u8>) a byte array.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub struct Base64Encoder<T>(pub T);

impl<T> Base64Encoder<T> {
    pub fn new(value: T) -> Self {
        value.into()
    }
}

impl<T> From<T> for Base64Encoder<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<R, T: AsRef<R>> AsRef<R> for Base64Encoder<T> {
    fn as_ref(&self) -> &R {
        self.0.as_ref()
    }
}

impl<C, T: std::ops::Deref<Target = [u8]>> Encode<C> for Base64Encoder<T> {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        e.str(&base64::encode(self.0.as_ref()))?;
        Ok(())
    }
}

impl<'d, C, T: FromIterator<u8>> Decode<'d, C> for Base64Encoder<T> {
    fn decode(d: &mut Decoder<'d>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        let b64 = d.str()?;
        let bytes =
            base64::decode(b64).map_err(|e| minicbor::decode::Error::message(e.to_string()))?;

        Ok(Self(T::from_iter(bytes.into_iter())))
    }
}

/// A structure that wraps a T in a CBOR ByteString.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct WrappedCbor<T>(T);

impl<T> From<T> for WrappedCbor<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<C, T> Encode<C> for WrappedCbor<T>
where
    T: Encode<C>,
{
    fn encode<W: Write>(&self, e: &mut Encoder<W>, ctx: &mut C) -> Result<(), Error<W::Error>> {
        let bytes = minicbor::to_vec_with(&self.0, ctx).unwrap();
        e.bytes(&bytes)?;
        Ok(())
    }
}

impl<'d, C, T: Decode<'d, C> + Sized> Decode<'d, C> for WrappedCbor<T>
where
    Self: 'd,
{
    fn decode(d: &mut Decoder<'d>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let bytes = d.bytes()?;
        let t: T = minicbor::decode_with(bytes, ctx)?;
        Ok(Self(t))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn cbor_null() {
        let null = CborNull;
        let enc = minicbor::to_vec(null).unwrap();
        // f6 (22) == null
        // See https://www.rfc-editor.org/rfc/rfc8949.html#fpnoconttbl2
        assert_eq!(hex::encode(enc), "f6");
    }

    /// Generate arbitraty CborAny value.
    ///
    /// Recursive structures depth, size and branch size are limited
    #[cfg(feature = "proptest")]
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

#[test]
fn base64_encoder_works() {
    let value: Vec<u8> = vec![1, 2, 3];
    let v = Base64Encoder::from(value.clone());

    let bytes = minicbor::to_vec(v).unwrap();
    // Read a string from it.
    let str: &str = minicbor::decode(&bytes).unwrap();

    let v2: Base64Encoder<Vec<u8>> = minicbor::decode(&bytes).unwrap();

    assert_eq!(value, v2.0);
    assert_eq!(str, "AQID");
}

#[cfg(feature = "proptest")]
proptest::proptest! {
    // No need to waste time on these tests.
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(10))]

    #[test]
    fn wrapped_cbor_works(value in tests::arb_cbor()) {
        let wrapped = WrappedCbor::from(value);

        let bytes = minicbor::to_vec(&wrapped).unwrap();
        let wrapped2 = minicbor::decode::<WrappedCbor<CborAny>>(&bytes).unwrap();

        assert_eq!(wrapped, wrapped2);
        // Check that bytes are actually a bstr.
        let d = minicbor::Decoder::new(&bytes);
        assert_eq!(d.datatype().unwrap(), minicbor::data::Type::Bytes);
    }
}
