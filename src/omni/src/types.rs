use crate::Identity;
use minicbor::data::{Tag, Type};
use minicbor::encode::{Error, Write};
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use num_bigint::BigUint;
use num_traits::Num;
use serde::Deserialize;
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Bound, RangeBounds};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod identity;

/// A deterministic (fixed point) percent value that can be multiplied with
/// numbers and rounded down.
pub struct Percent(pub fixed::types::U32F32);

impl Percent {
    pub fn new(i: u32, fraction: u32) -> Self {
        Self(fixed::types::U32F32::from_bits(
            (i as u64) << 32 + (fraction as u64),
        ))
    }

    pub fn apply(&self, n: TokenAmount) -> TokenAmount {
        let mut n: num_bigint::BigUint = n.into();
        n = n * self.0.to_bits() >> 32;
        n.into()
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

/// A Symbol is represented by a non-anonymous identity.
pub type Symbol = crate::Identity;

/// Transaction fees.
#[derive(Encode, Decode)]
pub struct TransactionFee {
    #[n(0)]
    pub fixed: Option<TokenAmount>,
    #[n(1)]
    pub percent: Option<Percent>,
}

impl TransactionFee {
    /// Calculates the actual fees of a transaction. The returned amount is the
    /// fees calculated, and not (amount + fees).
    pub fn calculate_fees(&self, amount: TokenAmount) -> TokenAmount {
        let mut fees = self.fixed.clone().unwrap_or_default();
        fees += if let Some(ref p) = self.percent {
            p.apply(amount)
        } else {
            TokenAmount::zero()
        };
        fees
    }
}

type TokenAmountStorage = num_bigint::BigUint;

#[repr(transparent)]
#[derive(Debug, Default, Hash, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct TokenAmount(TokenAmountStorage);

impl TokenAmount {
    pub fn zero() -> Self {
        Self(0u8.into())
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0u8.into()
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_bytes_be()
    }
}

impl From<u64> for TokenAmount {
    fn from(v: u64) -> Self {
        TokenAmount(v.into())
    }
}

impl From<u128> for TokenAmount {
    fn from(v: u128) -> Self {
        TokenAmount(v.into())
    }
}

impl From<Vec<u8>> for TokenAmount {
    fn from(v: Vec<u8>) -> Self {
        TokenAmount(num_bigint::BigUint::from_bytes_be(v.as_slice()))
    }
}

impl From<num_bigint::BigUint> for TokenAmount {
    fn from(v: BigUint) -> Self {
        TokenAmount(v)
    }
}

impl Into<num_bigint::BigUint> for TokenAmount {
    fn into(self) -> BigUint {
        self.0
    }
}

impl Display for TokenAmount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::ops::AddAssign for TokenAmount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl std::ops::SubAssign for TokenAmount {
    fn sub_assign(&mut self, rhs: Self) {
        if self.0 <= rhs.0 {
            self.0 = TokenAmountStorage::from(0u8);
        } else {
            self.0 -= rhs.0
        }
    }
}

impl Encode for TokenAmount {
    fn encode<W: encode::Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        use num_traits::cast::ToPrimitive;

        // Encode efficiently.
        if let Some(amount) = self.0.to_u64() {
            e.u64(amount)?;
        } else {
            e.tag(Tag::PosBignum)?.bytes(&self.0.to_bytes_be())?;
        }
        Ok(())
    }
}

impl<'b> Decode<'b> for TokenAmount {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        // Decode either.
        match d.datatype()? {
            Type::Tag => {
                if d.tag()? != Tag::PosBignum {
                    return Err(minicbor::decode::Error::Message("Invalid tag."));
                }

                let bytes = d.bytes()?.to_vec();
                Ok(TokenAmount::from(bytes))
            }
            _ => Ok(d.u64()?.into()),
        }
    }
}

impl<'de> Deserialize<'de> for TokenAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = TokenAmount;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("amount in number or string")
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TokenAmount(v.into()))
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let storage = TokenAmountStorage::from_str_radix(v, 10).map_err(E::custom)?;
                Ok(TokenAmount(storage))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Default, Debug)]
pub struct VecOrSingle<T>(pub Vec<T>);

impl<T> Into<Vec<T>> for VecOrSingle<T> {
    fn into(self) -> Vec<T> {
        self.0
    }
}
impl<T> From<Vec<T>> for VecOrSingle<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
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
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Timestamp(pub SystemTime);

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

impl Into<SystemTime> for Timestamp {
    fn into(self) -> SystemTime {
        self.0
    }
}

#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
#[repr(transparent)]
pub struct TransactionId(pub u64);

impl Encode for TransactionId {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        e.u64(self.0)?;
        Ok(())
    }
}

impl<'b> Decode<'b> for TransactionId {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        Ok(TransactionId(d.u64()?))
    }
}

impl Into<Vec<u8>> for TransactionId {
    fn into(self) -> Vec<u8> {
        self.0.to_be_bytes().to_vec()
    }
}

impl std::ops::Add<u64> for TransactionId {
    type Output = TransactionId;

    fn add(self, rhs: u64) -> Self::Output {
        TransactionId(self.0 + rhs)
    }
}

impl std::ops::Sub<u64> for TransactionId {
    type Output = TransactionId;

    fn sub(self, rhs: u64) -> Self::Output {
        TransactionId(self.0 - rhs)
    }
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum TransactionKind {
    Send = 0,
    Mint,
    Burn,
}

impl Encode for TransactionKind {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        e.u8(*self as u8)?;
        Ok(())
    }
}

impl<'b> Decode<'b> for TransactionKind {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        Ok(match d.u8()? {
            0 => Self::Send,
            1 => Self::Mint,
            2 => Self::Burn,
            _ => {
                return Err(minicbor::decode::Error::Message("Invalid TransactionKind."));
            }
        })
    }
}

#[derive(Encode, Decode)]
#[cbor(map)]
pub struct Transaction {
    #[n(0)]
    pub id: TransactionId,

    #[n(1)]
    pub time: Timestamp,

    #[n(2)]
    pub content: TransactionContent,
}

impl Transaction {
    pub fn send(
        id: TransactionId,
        time: SystemTime,
        from: Identity,
        to: Identity,
        symbol: String,
        amount: TokenAmount,
    ) -> Self {
        Transaction {
            id,
            time: time.into(),
            content: TransactionContent::Send {
                from,
                to,
                symbol,
                amount,
            },
        }
    }

    pub fn mint(
        id: TransactionId,
        time: SystemTime,
        account: Identity,
        symbol: String,
        amount: TokenAmount,
    ) -> Self {
        Transaction {
            id,
            time: time.into(),
            content: TransactionContent::Mint {
                account,
                symbol,
                amount,
            },
        }
    }

    pub fn burn(
        id: TransactionId,
        time: SystemTime,
        account: Identity,
        symbol: String,
        amount: TokenAmount,
    ) -> Self {
        Transaction {
            id,
            time: time.into(),
            content: TransactionContent::Burn {
                account,
                symbol,
                amount,
            },
        }
    }

    pub fn kind(&self) -> TransactionKind {
        match self.content {
            TransactionContent::Send { .. } => TransactionKind::Send,
            TransactionContent::Mint { .. } => TransactionKind::Mint,
            TransactionContent::Burn { .. } => TransactionKind::Burn,
        }
    }

    pub fn symbol(&self) -> &String {
        match &self.content {
            TransactionContent::Send { symbol, .. } => symbol,
            TransactionContent::Mint { symbol, .. } => symbol,
            TransactionContent::Burn { symbol, .. } => symbol,
        }
    }

    pub fn is_about(&self, id: &Identity) -> bool {
        match &self.content {
            TransactionContent::Send { from, to, .. } => id == from || id == to,
            TransactionContent::Mint { account, .. } => id == account,
            TransactionContent::Burn { account, .. } => id == account,
        }
    }
}

pub enum TransactionContent {
    Send {
        from: Identity,
        to: Identity,
        symbol: String,
        amount: TokenAmount,
    },
    Mint {
        account: Identity,
        symbol: String,
        amount: TokenAmount,
    },
    Burn {
        account: Identity,
        symbol: String,
        amount: TokenAmount,
    },
}

impl Encode for TransactionContent {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        match self {
            TransactionContent::Send {
                from,
                to,
                symbol,
                amount,
            } => {
                e.array(5)?
                    .u8(TransactionKind::Send as u8)?
                    .encode(from)?
                    .encode(to)?
                    .encode(symbol)?
                    .encode(amount)?;
            }
            TransactionContent::Mint {
                account,
                symbol,
                amount,
            } => {
                e.array(4)?
                    .u8(TransactionKind::Mint as u8)?
                    .encode(account)?
                    .encode(symbol)?
                    .encode(amount)?;
            }
            TransactionContent::Burn {
                account,
                symbol,
                amount,
            } => {
                e.array(4)?
                    .u8(TransactionKind::Burn as u8)?
                    .encode(account)?
                    .encode(symbol)?
                    .encode(amount)?;
            }
        }
        Ok(())
    }
}

impl<'b> Decode<'b> for TransactionContent {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let mut len = d.array()?;
        let content = match d.u8()? {
            0 => {
                // TransactionKind::Send
                len = len.map(|x| x - 5);
                TransactionContent::Send {
                    from: d.decode()?,
                    to: d.decode()?,
                    symbol: d.decode()?,
                    amount: d.decode()?,
                }
            }
            1 => {
                // TransactionKind::Mint
                len = len.map(|x| x - 4);
                TransactionContent::Mint {
                    account: d.decode()?,
                    symbol: d.decode()?,
                    amount: d.decode()?,
                }
            }
            2 => {
                // TransactionKind::Burn
                len = len.map(|x| x - 4);
                TransactionContent::Burn {
                    account: d.decode()?,
                    symbol: d.decode()?,
                    amount: d.decode()?,
                }
            }
            _ => return Err(minicbor::decode::Error::Message("Invalid TransactionKind")),
        };

        match len {
            Some(0) => Ok(content),
            None if d.datatype()? == minicbor::data::Type::Break => Ok(content),
            _ => Err(minicbor::decode::Error::Message(
                "Invalid TransactionContent array.",
            )),
        }
    }
}

#[derive(Copy, Clone)]
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
        fn encode_bound<'a, T: Encode, W: Write>(
            b: &Bound<T>,
            e: &'a mut Encoder<W>,
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
    pub kind: Option<VecOrSingle<TransactionKind>>,

    #[n(2)]
    pub symbol: Option<VecOrSingle<String>>,

    #[n(3)]
    pub id_range: Option<CborRange<TransactionId>>,

    #[n(4)]
    pub date_range: Option<CborRange<Timestamp>>,
}

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
            x => return Err(decode::Error::UnknownVariant(x as u32)),
        })
    }
}
