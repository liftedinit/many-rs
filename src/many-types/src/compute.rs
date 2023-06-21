use minicbor::encode::Write;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use strum::Display;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComputeStatus {
    Running = 0,
    Closed = 1,
}

impl<C> Encode<C> for ComputeStatus {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.u8(match self {
            ComputeStatus::Running => 0,
            ComputeStatus::Closed => 1,
        })?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for ComputeStatus {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        Ok(match d.u8()? {
            0 => Self::Running,
            1 => Self::Closed,
            x => return Err(decode::Error::unknown_variant(u32::from(x))),
        })
    }
}

const K: u64 = 1000;
const KI: u64 = 1024;
const M: u64 = 1000 * 1000;
const MI: u64 = 1024 * 1024;
const G: u64 = 1000 * 1000 * 1000;
const GI: u64 = 1024 * 1024 * 1024;
const T: u64 = 1000 * 1000 * 1000 * 1000;
const TI: u64 = 1024 * 1024 * 1024 * 1024;
const P: u64 = 1000 * 1000 * 1000 * 1000 * 1000;
const PI: u64 = 1024 * 1024 * 1024 * 1024 * 1024;
const E: u64 = 1000 * 1000 * 1000 * 1000 * 1000 * 1000;
const EI: u64 = 1024 * 1024 * 1024 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Display, Eq, PartialEq)]
#[strum(serialize_all = "PascalCase")]
pub enum ByteUnits {
    K = 0,
    KI,
    M,
    MI,
    G,
    GI,
    T,
    TI,
    P,
    PI,
    E,
    EI,
}

// Implement Encode and Decode for ByteUnits
impl<C> Encode<C> for ByteUnits {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.u64(match self {
            ByteUnits::K => K,
            ByteUnits::KI => KI,
            ByteUnits::M => M,
            ByteUnits::MI => MI,
            ByteUnits::G => G,
            ByteUnits::GI => GI,
            ByteUnits::T => T,
            ByteUnits::TI => TI,
            ByteUnits::P => P,
            ByteUnits::PI => PI,
            ByteUnits::E => E,
            ByteUnits::EI => EI,
        })?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for ByteUnits {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        Ok(match d.u64()? {
            0 => ByteUnits::K,
            1 => ByteUnits::KI,
            2 => ByteUnits::M,
            3 => ByteUnits::MI,
            4 => ByteUnits::G,
            5 => ByteUnits::GI,
            6 => ByteUnits::T,
            7 => ByteUnits::TI,
            8 => ByteUnits::P,
            9 => ByteUnits::PI,
            10 => ByteUnits::E,
            11 => ByteUnits::EI,
            x => return Err(decode::Error::message(format!("Unknown variant: {x}"))),
        })
    }
}

#[derive(Clone, Debug, Display, Eq, PartialEq)]
#[strum(serialize_all = "kebab-case")]
pub enum Region {
    UsEast = 0,
    UsWest = 1,
}

impl<C> Encode<C> for Region {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.u8(match self {
            Region::UsEast => 0,
            Region::UsWest => 1,
        })?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for Region {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        Ok(match d.u8()? {
            0 => Self::UsEast,
            1 => Self::UsWest,
            x => return Err(decode::Error::unknown_variant(u32::from(x))),
        })
    }
}

#[derive(Clone, Debug, Display, Eq, PartialEq)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Protocol {
    TCP = 0,
    UDP = 1,
}

impl<C> Encode<C> for Protocol {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.u8(match self {
            Protocol::TCP => 0,
            Protocol::UDP => 1,
        })?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for Protocol {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        Ok(match d.u8()? {
            0 => Self::TCP,
            1 => Self::UDP,
            x => return Err(decode::Error::unknown_variant(u32::from(x))),
        })
    }
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub struct ProviderInfo {
    pub host: String, // TODO: ManyUrl
    pub port: u16,
    pub external_port: u16,
    pub protocol: Protocol,
}