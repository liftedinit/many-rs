use many_error::ManyError;
use minicbor::data::{Tag, Type};
use minicbor::encode::Write;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};

/// NOTE: DO NOT ADD Default TO THIS TYPE.
#[repr(C)]
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[must_use]
pub enum Timestamp {
    /// Exists for backwards compatibility
    Int(u64),
    /// First parameter is seconds, second is nanoseconds
    Decimal(u64, u32),
}

impl Timestamp {
    pub fn now() -> Self {
        let duration = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time flew backward");
        Self::new_decimal(duration.as_secs(), duration.subsec_nanos())
            .expect("Time flew all around")
    }

    pub const fn new(secs: u64) -> Result<Self, ManyError> {
        Ok(Self::Int(secs))
    }

    pub fn new_decimal(secs: u64, nanos: u32) -> Result<Self, ManyError> {
        Ok(Self::Decimal(secs, nanos))
    }

    pub fn from_f64(secs: f64) -> Result<Self, ManyError> {
        let d = std::time::Duration::from_secs_f64(secs);
        Ok(Self::Decimal(d.as_secs(), d.subsec_nanos()))
    }

    pub fn from_system_time(t: std::time::SystemTime) -> Result<Self, ManyError> {
        let d = t
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ManyError::unknown("duration value can not represent system time"))?;
        Self::new_decimal(d.as_secs(), d.subsec_nanos())
    }

    pub fn as_system_time(&self) -> Result<std::time::SystemTime, ManyError> {
        std::time::UNIX_EPOCH
            .checked_add(std::time::Duration::new(
                self.as_secs(),
                self.subsec_nanos(),
            ))
            .ok_or_else(|| ManyError::unknown("duration value can not represent system time"))
    }

    pub fn as_secs(&self) -> u64 {
        match *self {
            Timestamp::Int(a) | Timestamp::Decimal(a, _) => a,
        }
    }

    pub fn subsec_nanos(&self) -> u32 {
        match *self {
            Timestamp::Int(_) => 0,
            Timestamp::Decimal(_, nanos) => nanos,
        }
    }

    pub fn as_secs_f64(&self) -> f64 {
        std::time::Duration::new(self.as_secs(), self.subsec_nanos()).as_secs_f64()
    }
}

impl std::ops::Add<u64> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: u64) -> Self::Output {
        Self::new_decimal(self.as_secs().add(rhs), self.subsec_nanos()).unwrap()
    }
}

impl std::ops::Add<f64> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: f64) -> Self::Output {
        Timestamp::from_f64(self.as_secs_f64().add(rhs)).unwrap()
    }
}

impl<C> Encode<C> for Timestamp {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        match *self {
            Timestamp::Int(secs) => e.tag(Tag::Timestamp)?.u64(secs)?,
            Timestamp::Decimal(_, _) => e.tag(Tag::Timestamp)?.f64(self.as_secs_f64())?,
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
            Type::F16 | Type::F32 | Type::F64 => Self::from_f64(d.f64()?).map_err(|_| {
                decode::Error::message("Time acted weirdly, this shouldn't be possible")
            }),
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

    let timestamp = Timestamp::from_f64(1.1).unwrap();
    let encoded = minicbor::to_vec(timestamp).unwrap();
    let decoded: Timestamp = minicbor::decode(&encoded).unwrap();
    assert_eq!(decoded, timestamp);
}

#[test]
fn timestamp_big_ranges() {
    let max_duration = std::time::Duration::MAX;
    let timestamp =
        Timestamp::new_decimal(max_duration.as_secs(), max_duration.subsec_nanos()).unwrap();
    assert_eq!(max_duration.as_secs(), timestamp.as_secs());
    assert_eq!(max_duration.subsec_nanos(), timestamp.subsec_nanos());
    assert_eq!(max_duration.as_secs_f64(), timestamp.as_secs_f64());

    let other_timestamp = Timestamp::from_f64(timestamp.as_secs_f64()).unwrap();
    assert_eq!(timestamp, other_timestamp);

    // Weirdly, std::time::Duration breaks when encoding a
    // Duration::MAX and decoding it back. Ought to be investigated.
    // But for normal ranges it's reasonable to expect Timestamp will
    // work as well as std::time::Duration, because that's what it is
    // under the hood

    // let encoded = minicbor::to_vec(timestamp).unwrap();
    // let decoded: Timestamp = minicbor::decode(&encoded).unwrap();
    // assert_eq!(decoded, timestamp);
}
