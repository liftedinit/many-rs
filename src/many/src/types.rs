use crate::{Identity, ManyError};
use minicbor::data::{Tag, Type};
use minicbor::encode::{Error, Write};
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use std::collections::BTreeSet;
use std::fmt::{Debug, Formatter};
use std::ops::{Bound, RangeBounds, Shl};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod blockchain;
pub mod identity;
pub mod ledger;

/// A deterministic (fixed point) percent value that can be multiplied with
/// numbers and rounded down.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Percent(pub fixed::types::U32F32);

impl Percent {
    pub fn new(i: u32, fraction: u32) -> Self {
        Self(fixed::types::U32F32::from_bits(
            u64::from(i).shl(32) + u64::from(fraction),
        ))
    }
}

impl Encode for Percent {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.u64(self.0.to_bits())?;
        Ok(())
    }
}

impl<'b> Decode<'b> for Percent {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        Ok(Self(fixed::types::U32F32::from_bits(d.u64()?)))
    }
}

#[derive(Clone, Default, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[must_use]
pub struct VecOrSingle<T>(pub Vec<T>);

impl<T> VecOrSingle<T> {
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }
}

impl<T> IntoIterator for VecOrSingle<T> {
    type Item = T;

    type IntoIter = <Vec<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T> From<VecOrSingle<T>> for Vec<T> {
    fn from(v: VecOrSingle<T>) -> Vec<T> {
        v.0
    }
}

impl<T> From<Vec<T>> for VecOrSingle<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
    }
}

impl<T: Ord> Into<BTreeSet<T>> for VecOrSingle<T> {
    fn into(self) -> BTreeSet<T> {
        BTreeSet::from_iter(self.into_iter())
    }
}

impl<T> Encode for VecOrSingle<T>
where
    T: Encode,
{
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        if self.0.len() == 1 {
            self.0.get(0).encode(e)
        } else {
            self.0.encode(e)
        }
    }
}

impl<'b, T> Decode<'b> for VecOrSingle<T>
where
    T: Decode<'b>,
{
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        Ok(match d.datatype()? {
            Type::Array | Type::ArrayIndef => Self(d.array_iter()?.collect::<Result<_, _>>()?),
            _ => Self(vec![d.decode::<T>()?]),
        })
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[must_use]
pub struct Timestamp(pub SystemTime);

impl Timestamp {
    pub fn now() -> Self {
        Self(SystemTime::now())
    }

    pub fn new(secs: u64) -> Result<Self, ManyError> {
        Ok(Self(
            UNIX_EPOCH
                .checked_add(Duration::new(secs, 0))
                .ok_or_else(|| {
                    ManyError::unknown("duration value can not represent system time".to_string())
                })?,
        ))
    }
}

impl Encode for Timestamp {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        e.tag(Tag::Timestamp)?.u64(
            self.0
                .duration_since(UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs(),
        )?;
        Ok(())
    }
}

impl<'b> Decode<'b> for Timestamp {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        if d.tag()? != Tag::Timestamp {
            return Err(decode::Error::Message("Invalid tag."));
        }

        let secs = d.u64()?;
        Ok(Self(
            UNIX_EPOCH
                .checked_add(Duration::from_secs(secs))
                .ok_or(decode::Error::Message(
                    "duration value can not represent system time",
                ))?,
        ))
    }
}

impl From<SystemTime> for Timestamp {
    fn from(t: SystemTime) -> Self {
        Self(t)
    }
}

impl From<Timestamp> for SystemTime {
    fn from(t: Timestamp) -> SystemTime {
        t.0
    }
}

#[derive(Copy, Clone)]
#[must_use]
pub struct CborRange<T> {
    pub start: std::ops::Bound<T>,
    pub end: std::ops::Bound<T>,
}

impl<T> RangeBounds<T> for CborRange<T> {
    fn start_bound(&self) -> Bound<&T> {
        match self.start {
            Bound::Included(ref x) => Bound::Included(x),
            Bound::Excluded(ref x) => Bound::Excluded(x),
            Bound::Unbounded => Bound::Unbounded,
        }
    }

    fn end_bound(&self) -> Bound<&T> {
        match self.end {
            Bound::Included(ref x) => Bound::Included(x),
            Bound::Excluded(ref x) => Bound::Excluded(x),
            Bound::Unbounded => Bound::Unbounded,
        }
    }

    fn contains<U>(&self, item: &U) -> bool
    where
        T: PartialOrd<U>,
        U: ?Sized + PartialOrd<T>,
    {
        (match self.start_bound() {
            Bound::Included(start) => start <= item,
            Bound::Excluded(start) => start < item,
            Bound::Unbounded => true,
        }) && (match self.end_bound() {
            Bound::Included(end) => item <= end,
            Bound::Excluded(end) => item < end,
            Bound::Unbounded => true,
        })
    }
}

impl<T: Debug> Debug for CborRange<T> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.start.fmt(fmt)?;
        write!(fmt, "..")?;
        self.end.fmt(fmt)?;
        Ok(())
    }
}

impl<T> Default for CborRange<T> {
    fn default() -> Self {
        Self {
            start: Bound::Unbounded,
            end: Bound::Unbounded,
        }
    }
}

impl<T: PartialOrd<T>> CborRange<T> {
    pub fn contains<U>(&self, item: &U) -> bool
    where
        T: PartialOrd<U>,
        U: ?Sized + PartialOrd<T>,
    {
        <Self as std::ops::RangeBounds<T>>::contains(self, item)
    }
}

impl<T> Encode for CborRange<T>
where
    T: Encode,
{
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        fn encode_bound<T: Encode, W: Write>(
            b: &Bound<T>,
            e: &mut Encoder<W>,
        ) -> Result<(), Error<W::Error>> {
            match b {
                Bound::Included(v) => {
                    e.array(2)?.u8(0)?.encode(v)?;
                }
                Bound::Excluded(v) => {
                    e.array(2)?.u8(1)?.encode(v)?;
                }
                Bound::Unbounded => {
                    e.array(0)?;
                }
            };
            Ok(())
        }

        match (&self.start, &self.end) {
            (Bound::Unbounded, Bound::Unbounded) => {
                e.map(0)?;
            }
            (st, Bound::Unbounded) => {
                e.map(1)?.u8(0)?;
                encode_bound(st, e)?;
            }
            (Bound::Unbounded, en) => {
                e.map(1)?.u8(1)?;
                encode_bound(en, e)?;
            }
            (st, en) => {
                e.map(2)?;
                e.u8(0)?;
                encode_bound(st, e)?;
                e.u8(1)?;
                encode_bound(en, e)?;
            }
        }

        Ok(())
    }
}

impl<'b, T: Decode<'b>> Decode<'b> for CborRange<T> {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        struct BoundDecoder<T>(pub Bound<T>);
        impl<'b, T: Decode<'b>> Decode<'b> for BoundDecoder<T> {
            fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
                let len = d.array()?;
                let bound = match len {
                    Some(x) => match x {
                        0 => Bound::Unbounded,
                        2 => match d.u32()? {
                            0 => Bound::Included(d.decode()?),
                            1 => Bound::Excluded(d.decode()?),
                            x => return Err(decode::Error::UnknownVariant(x)),
                        },
                        x => return Err(decode::Error::UnknownVariant(x as u32)),
                    },
                    None => return Err(decode::Error::TypeMismatch(Type::ArrayIndef, "Array")),
                };
                Ok(Self(bound))
            }
        }

        let mut start: Bound<T> = Bound::Unbounded;
        let mut end: Bound<T> = Bound::Unbounded;

        for item in d.map_iter()? {
            let (key, value) = item?;
            match key {
                0u8 => start = value,
                1u8 => end = value,
                _ => {}
            }
        }

        Ok(Self { start, end })
    }
}

#[derive(Default, Encode, Decode)]
#[cbor(map)]
pub struct TransactionFilter {
    #[n(0)]
    pub account: Option<VecOrSingle<Identity>>,

    #[n(1)]
    pub kind: Option<VecOrSingle<ledger::TransactionKind>>,

    #[n(2)]
    pub symbol: Option<VecOrSingle<Identity>>,

    #[n(3)]
    pub id_range: Option<CborRange<ledger::TransactionId>>,

    #[n(4)]
    pub date_range: Option<CborRange<Timestamp>>,
}

#[must_use]
pub enum SortOrder {
    Indeterminate = 0,
    Ascending = 1,
    Descending = 2,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::Indeterminate
    }
}

impl Encode for SortOrder {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.u8(match self {
            SortOrder::Indeterminate => 0,
            SortOrder::Ascending => 1,
            SortOrder::Descending => 2,
        })?;
        Ok(())
    }
}

impl<'b> Decode<'b> for SortOrder {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        Ok(match d.u8()? {
            0 => Self::Indeterminate,
            1 => Self::Ascending,
            2 => Self::Descending,
            x => return Err(decode::Error::UnknownVariant(u32::from(x))),
        })
    }
}
