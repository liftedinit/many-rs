use many_error::ManyError;
use minicbor::{
    data::{Tag, Type},
    decode,
    encode::{self, Write},
    Decode, Decoder, Encode, Encoder,
};

/// NOTE: DO NOT ADD Default TO THIS TYPE.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialOrd, PartialEq)]
#[must_use]
pub enum Timestamp {
    Int(u64),
    Float(f64),
}

impl Timestamp {
    pub fn now() -> Self {
        Self::new_f64(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs_f64(),
        )
        .expect("Time flew all around")
    }

    pub const fn new(secs: u64) -> Result<Self, ManyError> {
        Ok(Self::Int(secs))
    }

    pub const fn new_f64(secs: f64) -> Result<Self, ManyError> {
        Ok(Self::Float(secs))
    }

    pub fn from_system_time(t: std::time::SystemTime) -> Result<Self, ManyError> {
        let d = t.duration_since(std::time::UNIX_EPOCH).map_err(|_| {
            ManyError::unknown("duration value can not represent system time".to_string())
        })?;
        Ok(Self::Float(d.as_secs_f64()))
    }

    pub fn as_system_time(&self) -> Result<std::time::SystemTime, ManyError> {
        std::time::UNIX_EPOCH
            .checked_add(std::time::Duration::new(self.secs(), self.nanos()))
            .ok_or_else(|| {
                ManyError::unknown("duration value can not represent system time".to_string())
            })
    }

    pub fn secs(&self) -> u64 {
        match self {
            Timestamp::Int(a) => *a,
            Timestamp::Float(a) => *a as u64,
        }
    }

    pub fn nanos(&self) -> u32 {
        match self {
            Timestamp::Int(_) => 0,
            Timestamp::Float(a) => (a.fract() * 1_000_000_000.0) as u32,
        }
    }

    pub fn secs_f64(&self) -> f64 {
        match self {
            Timestamp::Int(a) => *a as f64,
            Timestamp::Float(a) => *a,
        }
    }
}

impl std::ops::Add<u64> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: u64) -> Self::Output {
        match self {
            Timestamp::Int(a) => Timestamp::new(a.add(&rhs)).unwrap(),
            Timestamp::Float(a) => Timestamp::new_f64(a.add(&(rhs as f64))).unwrap(),
        }
    }
}

impl std::ops::Add<f64> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: f64) -> Self::Output {
        Timestamp::new_f64(self.secs_f64().add(&rhs)).unwrap()
    }
}

impl<C> Encode<C> for Timestamp {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        match self {
            Timestamp::Int(a) => e.tag(Tag::Timestamp)?.u64(*a)?,
            Timestamp::Float(a) => e.tag(Tag::Timestamp)?.f64(*a)?,
        };
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for Timestamp {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        if d.tag()? != Tag::Timestamp {
            return Err(decode::Error::message("Invalid tag."));
        }

        match d.datatype()? {
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => Ok(Self::Int(d.u64()?)),
            Type::F16 | Type::F32 | Type::F64 => Ok(Self::Float(d.f64()?)),
            t => Err(decode::Error::type_mismatch(t)),
        }
    }
}

#[test]
fn timestamp_encode_decode_works() {
    let timestamp = Timestamp::new(10).unwrap();
    let encoded = minicbor::to_vec(timestamp).unwrap();
    let decoded: Timestamp = minicbor::decode(&encoded).unwrap();
    assert_eq!(decoded, timestamp);

    let timestamp = Timestamp::new_f64(1.1).unwrap();
    let encoded = minicbor::to_vec(timestamp).unwrap();
    let decoded: Timestamp = minicbor::decode(&encoded).unwrap();
    assert_eq!(decoded, timestamp);
}
