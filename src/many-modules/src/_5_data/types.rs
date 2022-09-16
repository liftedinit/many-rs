use many_types::AttributeRelatedIndex;
use minicbor::{Decode, Encode};
use num_bigint::BigInt;

pub type DataIndex = AttributeRelatedIndex;

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub enum DataType {
    #[n(0)]
    Counter,
    #[n(1)]
    Gauge,
}

#[derive(Clone, Decode, Encode, Debug)]
pub enum DataValue {
    #[n(0)]
    Counter(#[n(0)] DataValueTypeCounter),
    #[n(1)]
    Gauge(#[n(0)] DataValueTypeGauge),
}

impl TryFrom<DataValue> for BigInt {
    type Error = String;

    fn try_from(value: DataValue) -> Result<Self, Self::Error> {
        match value {
            DataValue::Counter(c) => Ok(c.into()),
            DataValue::Gauge(g) => g.try_into(),
        }
    }
}

pub type DataValueTypeCounter = u64;

#[derive(Clone, Decode, Encode, Debug)]
pub enum DataValueTypeGauge {
    #[n(0)]
    Int(#[n(0)] i64),
    #[n(1)]
    Float(#[n(0)] f64),
    #[n(2)]
    BigInt(#[cbor(n(0), decode_with = "decode_bigint", encode_with = "encode_bigint")] BigInt),
}

impl TryFrom<DataValueTypeGauge> for BigInt {
    type Error = String;

    fn try_from(value: DataValueTypeGauge) -> Result<Self, Self::Error> {
        match value {
            DataValueTypeGauge::Int(i) => Ok(i.into()),
            DataValueTypeGauge::BigInt(b) => Ok(b),
            DataValueTypeGauge::Float(_) => {
                Err("Floats can't be converted to BigInt without loss".into())
            }
        }
    }
}

fn decode_bigint<C>(
    d: &mut minicbor::Decoder<'_>,
    _: &mut C,
) -> Result<BigInt, minicbor::decode::Error> {
    let vec: Vec<u8> = d.decode()?;
    Ok(BigInt::from_signed_bytes_be(vec.as_slice()))
}

fn encode_bigint<C, W: minicbor::encode::Write>(
    v: &BigInt,
    e: &mut minicbor::Encoder<W>,
    _: &mut C,
) -> Result<(), minicbor::encode::Error<W::Error>> {
    e.encode(v.to_signed_bytes_be())?;
    Ok(())
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
pub struct DataInfo {
    #[n(0)]
    pub r#type: DataType,
    #[n(1)]
    pub shortname: String,
}
