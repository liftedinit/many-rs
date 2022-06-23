use crate::protocol::AttributeId;
use crate::ManyError;
use minicbor::data::{Tag, Type};
use minicbor::encode::{Error, Write};
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use std::collections::BTreeSet;
use std::fmt::{Debug, Formatter};
use std::ops::{Bound, RangeBounds, Shl};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod blockchain;
pub mod either;
pub mod events;
pub mod identity;
pub mod ledger;

pub use either::Either;

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

#[derive(Clone, Default, Debug, PartialEq)]
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

impl<T: Ord> From<VecOrSingle<T>> for BTreeSet<T> {
    fn from(v: VecOrSingle<T>) -> BTreeSet<T> {
        BTreeSet::from_iter(v.into_iter())
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
        Self::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs(),
        )
        .expect("Time flew all around")
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

#[derive(Copy, Clone, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum AttributeRelatedIndexInner {
    None,
    One([u32; 1]),
    Two([u32; 2]),
    Three([u32; 3]),
    Four([u32; 4]),
}

impl Default for AttributeRelatedIndexInner {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[must_use]
pub struct AttributeRelatedIndex {
    pub attribute: AttributeId,
    indices: AttributeRelatedIndexInner,
}

impl AttributeRelatedIndex {
    pub fn new(attribute: AttributeId) -> Self {
        Self {
            attribute,
            indices: AttributeRelatedIndexInner::default(),
        }
    }

    pub fn with_index(self, index: u32) -> Self {
        let indices = match self.indices {
            AttributeRelatedIndexInner::None => AttributeRelatedIndexInner::One([index]),
            AttributeRelatedIndexInner::One(a) => AttributeRelatedIndexInner::Two([a[0], index]),
            AttributeRelatedIndexInner::Two(a) => {
                AttributeRelatedIndexInner::Three([a[0], a[1], index])
            }
            AttributeRelatedIndexInner::Three(a) => {
                AttributeRelatedIndexInner::Four([a[0], a[1], a[2], index])
            }
            AttributeRelatedIndexInner::Four(a) => AttributeRelatedIndexInner::Four(a),
        };

        Self {
            attribute: self.attribute,
            indices,
        }
    }

    pub fn indices(&self) -> &[u32] {
        match &self.indices {
            AttributeRelatedIndexInner::None => &[],
            AttributeRelatedIndexInner::One(a) => a,
            AttributeRelatedIndexInner::Two(a) => a,
            AttributeRelatedIndexInner::Three(a) => a,
            AttributeRelatedIndexInner::Four(a) => a,
        }
    }

    pub fn flattened(&self) -> Vec<u32> {
        [&[self.attribute], self.indices()].concat().to_vec()
    }
}

impl Debug for AttributeRelatedIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tuple = f.debug_tuple("AttributeRelatedIndex");

        tuple.field(&self.attribute);

        for x in self.indices() {
            tuple.field(x);
        }

        tuple.finish()
    }
}

impl Encode for AttributeRelatedIndex {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        match self.indices() {
            [] => {
                e.encode(self.attribute)?;
            }
            a => {
                e.array(2)?.encode(self.attribute)?;

                let mut chunks = a.chunks(2).peekable();
                while let Some(chunk) = chunks.next() {
                    match chunk {
                        [a] => {
                            e.u32(*a)?;
                        }
                        [a, b] => {
                            if chunks.peek().is_none() {
                                e.array(2)?.u32(*a)?.u32(*b)?;
                            } else {
                                e.array(2)?.u32(chunk[0])?.array(2)?.u32(chunk[1])?;
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        Ok(())
    }
}

impl<'b> Decode<'b> for AttributeRelatedIndex {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        let mut index = match d.datatype()? {
            Type::Array => match d.array()? {
                Some(x) if x == 2 => Self::new(d.decode()?),
                _ => return Err(decode::Error::Message("Expected array of 2 elements")),
            },
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => return Ok(Self::new(d.decode()?)),
            x => return Err(decode::Error::TypeMismatch(x, "array or attribute id")),
        };

        loop {
            index = match d.datatype()? {
                Type::Array => match d.array()? {
                    Some(x) if x == 2 => index.with_index(d.decode()?),
                    _ => return Err(decode::Error::Message("Expected array of 2 elements")),
                },
                Type::U8 | Type::U16 | Type::U32 | Type::U64 => {
                    return Ok(index.with_index(d.decode()?))
                }
                x => return Err(decode::Error::TypeMismatch(x, "array or uint")),
            };
        }
    }
}

#[test]
fn attribute_related_index_encode_0() {
    let i = AttributeRelatedIndex::new(1);
    let b = minicbor::to_vec(i).unwrap();
    assert_eq!(minicbor::display(&b).to_string(), "1");
    assert_eq!(minicbor::decode::<AttributeRelatedIndex>(&b).unwrap(), i);
}

#[test]
fn attribute_related_index_encode_1() {
    let i = AttributeRelatedIndex::new(2).with_index(3);
    let b = minicbor::to_vec(i).unwrap();
    assert_eq!(minicbor::display(&b).to_string(), "[2, 3]");
    assert_eq!(minicbor::decode::<AttributeRelatedIndex>(&b).unwrap(), i);
}

#[test]
fn attribute_related_index_encode_2() {
    let i = AttributeRelatedIndex::new(4).with_index(5).with_index(6);
    let b = minicbor::to_vec(i).unwrap();
    assert_eq!(minicbor::display(&b).to_string(), "[4, [5, 6]]");
    assert_eq!(minicbor::decode::<AttributeRelatedIndex>(&b).unwrap(), i);
}

#[test]
fn attribute_related_index_encode_3() {
    let i = AttributeRelatedIndex::new(7)
        .with_index(8)
        .with_index(9)
        .with_index(10);
    let b = minicbor::to_vec(i).unwrap();
    assert_eq!(minicbor::display(&b).to_string(), "[7, [8, [9, 10]]]");
    assert_eq!(minicbor::decode::<AttributeRelatedIndex>(&b).unwrap(), i);
}

#[test]
fn attribute_related_index_encode_4() {
    let i = AttributeRelatedIndex::new(11)
        .with_index(12)
        .with_index(13)
        .with_index(14)
        .with_index(15);
    let b = minicbor::to_vec(i).unwrap();
    assert_eq!(
        minicbor::display(&b).to_string(),
        "[11, [12, [13, [14, 15]]]]"
    );
    assert_eq!(minicbor::decode::<AttributeRelatedIndex>(&b).unwrap(), i);
}

#[test]
fn attribute_related_index_encode_5() {
    // Should ignore the fifth index, as we only support 4.
    let i = AttributeRelatedIndex::new(16)
        .with_index(17)
        .with_index(18)
        .with_index(19)
        .with_index(20)
        .with_index(21);
    let b = minicbor::to_vec(i).unwrap();
    assert_eq!(
        minicbor::display(&b).to_string(),
        "[16, [17, [18, [19, 20]]]]"
    );
    assert_eq!(minicbor::decode::<AttributeRelatedIndex>(&b).unwrap(), i);
}

#[test]
fn either_works() {
    type EitherTest = Either<bool, u32>;

    assert_eq!(
        minicbor::decode::<EitherTest>(&[0]).unwrap(),
        EitherTest::Right(0)
    );
    assert_eq!(
        minicbor::decode::<EitherTest>(&[0xF4]).unwrap(),
        EitherTest::Left(false)
    );
    assert_eq!(
        minicbor::decode::<EitherTest>(&[0x1A, 0x00, 0x0F, 0x42, 0x40]).unwrap(),
        EitherTest::Right(1_000_000)
    );

    assert_eq!(
        &minicbor::to_vec(EitherTest::Right(1_000_000)).unwrap(),
        &[0x1A, 0x00, 0x0F, 0x42, 0x40]
    );
    assert_eq!(&minicbor::to_vec(EitherTest::Left(true)).unwrap(), &[0xF5]);
}
