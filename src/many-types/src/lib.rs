use many_error::ManyError;
use minicbor::data::{Tag, Type};
use minicbor::encode::{Error, Write};
use minicbor::{decode, Decode, Decoder, Encode, Encoder};
use std::collections::BTreeSet;
use std::fmt::{Debug, Formatter};
use std::ops::{Bound, RangeBounds, Shl};

pub mod attributes;
pub mod blockchain;
pub mod cbor;
pub mod either;
pub mod identity {
    pub use many_identity::*;
}
pub mod ledger;
pub mod memo;
pub mod proof;

use attributes::AttributeId;
pub use either::Either;
pub use memo::Memo;
pub use proof::{ProofOperation, PROOF};

pub mod legacy {
    pub use crate::memo::DataLegacy;
    pub use crate::memo::MemoLegacy;
}

/// A simple macro to create CBOR types. This only covers a few cases but will
/// save a lot of boilerplate for those cases. Any use case that isn't covered
/// by this macro should simply be implemented on its own.
/// TODO: the next step to improve this is to have a proc_macro that reads the
///       CDDL directly.
#[macro_export]
macro_rules! cbor_type_decl {
    (
        $(
            $vis: vis struct $name: ident {
                $(
                    $($tag: ident)* $fidx: literal => $fname: ident: $ftype: ty
                ),+ $(,)?
            }
        )*
    ) => {
        $(
            #[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
            #[cfg_attr(feature = "cucumber", derive(Default))]
            #[cbor(map)]
            $vis struct $name {
                $(
                    #[n( $fidx )] pub $fname: $ftype,
                )+
            }
        )*
    };
}

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

impl<C> Encode<C> for Percent {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        e.u64(self.0.to_bits())?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for Percent {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        Ok(Self(fixed::types::U32F32::from_bits(d.u64()?)))
    }
}

/// Equivalent to enum VecOrSingle<T> { Single(T), Vec(Vec<T>) } in
/// the cbor-level. That is, a user can decode a VecOrSingle from
/// either a single value or a vec.
#[derive(Clone, Default, Debug, Eq, PartialEq)]
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

impl<T, C> Encode<C> for VecOrSingle<T>
where
    T: Encode<C>,
{
    fn encode<W: Write>(&self, e: &mut Encoder<W>, ctx: &mut C) -> Result<(), Error<W::Error>> {
        if self.0.len() == 1 {
            self.0.get(0).encode(e, ctx)
        } else {
            self.0.encode(e, ctx)
        }
    }
}

impl<'b, T, C> Decode<'b, C> for VecOrSingle<T>
where
    T: Decode<'b, C>,
{
    fn decode(d: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, decode::Error> {
        Ok(match d.datatype()? {
            Type::Array | Type::ArrayIndef => {
                Self(d.array_iter_with(ctx)?.collect::<Result<_, _>>()?)
            }
            _ => Self(vec![d.decode_with::<C, T>(ctx)?]),
        })
    }
}

/// NOTE: DO NOT ADD Default TO THIS TYPE.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[must_use]
pub struct Timestamp(u64);

impl Timestamp {
    pub fn now() -> Self {
        Self::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs(),
        )
        .expect("Time flew all around")
    }

    pub const fn new(secs: u64) -> Result<Self, ManyError> {
        Ok(Self(secs))
    }

    pub fn from_system_time(t: std::time::SystemTime) -> Result<Self, ManyError> {
        let d = t.duration_since(std::time::UNIX_EPOCH).map_err(|_| {
            ManyError::unknown("duration value can not represent system time".to_string())
        })?;
        Ok(Self(d.as_secs()))
    }

    pub fn as_system_time(&self) -> Result<std::time::SystemTime, ManyError> {
        std::time::UNIX_EPOCH
            .checked_add(std::time::Duration::new(self.0, 0))
            .ok_or_else(|| {
                ManyError::unknown("duration value can not represent system time".to_string())
            })
    }

    pub fn secs(&self) -> u64 {
        self.0
    }
}

impl std::ops::Add<u64> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: u64) -> Self::Output {
        Timestamp::new(self.0.add(&rhs)).unwrap()
    }
}

impl<C> Encode<C> for Timestamp {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        e.tag(Tag::Timestamp)?.u64(self.0)?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for Timestamp {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        if d.tag()? != Tag::Timestamp {
            return Err(decode::Error::message("Invalid tag."));
        }

        let secs = d.u64()?;
        Ok(Self(secs))
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[must_use]
pub struct CborRange<T> {
    pub start: Bound<T>,
    pub end: Bound<T>,
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
        <Self as RangeBounds<T>>::contains(self, item)
    }
}

impl<T, C> Encode<C> for CborRange<T>
where
    T: Encode<C>,
{
    fn encode<W: Write>(&self, e: &mut Encoder<W>, ctx: &mut C) -> Result<(), Error<W::Error>> {
        fn encode_bound<T: Encode<C>, W: Write, C>(
            b: &Bound<T>,
            e: &mut Encoder<W>,
            ctx: &mut C,
        ) -> Result<(), Error<W::Error>> {
            match b {
                Bound::Included(v) => {
                    e.array(2)?.u8(0)?.encode_with(v, ctx)?;
                }
                Bound::Excluded(v) => {
                    e.array(2)?.u8(1)?.encode_with(v, ctx)?;
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
                encode_bound(st, e, ctx)?;
            }
            (Bound::Unbounded, en) => {
                e.map(1)?.u8(1)?;
                encode_bound(en, e, ctx)?;
            }
            (st, en) => {
                e.map(2)?;
                e.u8(0)?;
                encode_bound(st, e, ctx)?;
                e.u8(1)?;
                encode_bound(en, e, ctx)?;
            }
        }

        Ok(())
    }
}

impl<'b, T: Decode<'b, C>, C> Decode<'b, C> for CborRange<T> {
    fn decode(d: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, decode::Error> {
        struct BoundDecoder<T>(pub Bound<T>);
        impl<'b, T: Decode<'b, C>, C> Decode<'b, C> for BoundDecoder<T> {
            fn decode(d: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, decode::Error> {
                let len = d.array()?;
                let bound = match len {
                    Some(x) => match x {
                        0 => Bound::Unbounded,
                        2 => match d.u32()? {
                            0 => Bound::Included(d.decode_with(ctx)?),
                            1 => Bound::Excluded(d.decode_with(ctx)?),
                            x => return Err(decode::Error::unknown_variant(x)),
                        },
                        x => return Err(decode::Error::unknown_variant(x as u32)),
                    },
                    None => return Err(decode::Error::type_mismatch(Type::ArrayIndef)),
                };
                Ok(Self(bound))
            }
        }

        let mut start: Bound<T> = Bound::Unbounded;
        let mut end: Bound<T> = Bound::Unbounded;

        for item in d.map_iter_with(ctx)? {
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

#[derive(Clone, Debug, Eq, PartialEq)]
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

impl<C> Encode<C> for SortOrder {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        e.u8(match self {
            SortOrder::Indeterminate => 0,
            SortOrder::Ascending => 1,
            SortOrder::Descending => 2,
        })?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for SortOrder {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        Ok(match d.u8()? {
            0 => Self::Indeterminate,
            1 => Self::Ascending,
            2 => Self::Descending,
            x => return Err(decode::Error::unknown_variant(u32::from(x))),
        })
    }
}

#[derive(Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq)]
enum AttributeRelatedIndexInner {
    #[default]
    None,
    One([u32; 1]),
    Two([u32; 2]),
    Three([u32; 3]),
    Four([u32; 4]),
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[must_use]
pub struct AttributeRelatedIndex {
    pub attribute: AttributeId,
    indices: AttributeRelatedIndexInner,
}

impl AttributeRelatedIndex {
    #[inline]
    pub const fn new(attribute: AttributeId) -> Self {
        Self {
            attribute,
            indices: AttributeRelatedIndexInner::None,
        }
    }

    #[inline]
    pub const fn with_index(self, index: u32) -> Self {
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

    pub const fn indices(&self) -> &[u32] {
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

impl<C> Encode<C> for AttributeRelatedIndex {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
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

impl<'b, C> Decode<'b, C> for AttributeRelatedIndex {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        let mut index = match d.datatype()? {
            Type::Array => match d.array()? {
                Some(x) if x == 2 => Self::new(d.decode()?),
                _ => return Err(decode::Error::message("Expected array of 2 elements")),
            },
            Type::U8 | Type::U16 | Type::U32 | Type::U64 => return Ok(Self::new(d.decode()?)),
            x => return Err(decode::Error::type_mismatch(x)),
        };

        loop {
            index = match d.datatype()? {
                Type::Array => match d.array()? {
                    Some(x) if x == 2 => index.with_index(d.decode()?),
                    _ => return Err(decode::Error::message("Expected array of 2 elements")),
                },
                Type::U8 | Type::U16 | Type::U32 | Type::U64 => {
                    return Ok(index.with_index(d.decode()?))
                }
                x => return Err(decode::Error::type_mismatch(x)),
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
