//! All data holders for holding data in a MANY framework server.
//!
//! A server/module is expected to have a single `DataSet` where each
//! submodule register their data types, and then can access it to increment
//! or change their values.
//!
//! Data values include the type expected of them, so DataSet is for storage,
//! and DataValue should be passed on the wire.
use many_error::ManyError;
use many_types::attributes::AttributeId;
use many_types::AttributeRelatedIndex;
use minicbor::data::{Tag, Type};
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use num_bigint::{BigInt, Sign};
use num_traits::cast::ToPrimitive;
use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard};

macro_rules! decl_type {
    ( ($index_name: ident, $info_name: ident) => ( $short: literal, [ $first: literal, $( $index: literal ),* ], counter ) ) => {
        pub const $index_name: DataIndex = DataIndex::new($first) $(.with_index($index) )*;
        pub const $info_name: DataInfo = DataInfo {
            r#type: DataType::Counter,
            shortname: std::borrow::Cow::Borrowed($short),
        };
    };
    ( ($index_name: ident, $info_name: ident) => ( $short: literal, [ $first: literal, $( $index: literal ),* ], gauge_int ) ) => {
        pub const $index_name: DataIndex = DataIndex::new($first) $(.with_index($index) )*;
        pub const $info_name: DataInfo = DataInfo {
            r#type: DataType::GaugeInt,
            shortname: std::borrow::Cow::Borrowed($short),
        };
    };
    ( $(
        mod $mod_name: ident {
            $( ($index_name: ident, $info_name: ident) => ( $short: literal, $index: tt, $ty: ident ) ),* $(,)?
        }
    ),+ $(,)? ) => {
        $(
        pub mod $mod_name {
            use super::{DataIndex, DataInfo, DataType};

            $(
                decl_type!( ($index_name, $info_name) => ( $short, $index, $ty ) );
            )+

            pub(crate) const ALL: &[(DataIndex, &DataInfo)] = &[
                $(
                    ($index_name, & $info_name)
                ),+
            ];
        }
        )+
    }
}

decl_type!(
    mod accounts_count {
        (TOTAL_COUNT_INDEX, TOTAL_COUNT_INFO) => ( "accountTotalCount", [0, 2, 0], gauge_int ),
        (NON_ZERO_TOTAL_COUNT_INDEX, NON_ZERO_TOTAL_COUNT_INFO) => ( "nonZeroAccountTotalCount", [0, 2, 1], gauge_int ),
    }
);

#[derive(Copy, Clone, Debug, Decode, Encode, PartialOrd, Ord, Eq, PartialEq)]
#[repr(transparent)]
#[cbor(transparent)]
pub struct DataIndex(#[n(0)] AttributeRelatedIndex);

impl DataIndex {
    #[inline]
    pub const fn new(attribute: AttributeId) -> Self {
        Self(AttributeRelatedIndex::new(attribute))
    }

    #[inline]
    pub const fn with_index(self, index: u32) -> Self {
        Self(self.0.with_index(index))
    }
}

impl<T> From<T> for DataIndex
where
    T: Into<AttributeRelatedIndex>,
{
    fn from(index: T) -> Self {
        Self(index.into())
    }
}

#[derive(Copy, Clone, Debug, Decode, Encode, Eq, PartialEq)]
#[cbor(index_only)]
#[repr(u32)]
pub enum DataType {
    #[n(10100)]
    Counter = 10100,
    #[n(10101)]
    GaugeInt = 10101,
}

#[derive(Clone, Debug, Default)]
pub struct DataSet {
    types: BTreeMap<DataIndex, DataInfo>,
    values: BTreeMap<DataIndex, Arc<RwLock<DataValue>>>,
}

impl DataSet {
    pub fn with_known_types(mut self) -> Result<Self, ManyError> {
        // Check for duplicate first (don't modify if fails).
        for (index, _info) in accounts_count::ALL {
            if self.types.contains_key(index) {
                return Err(ManyError::unknown("Type already registered."));
            }
        }
        for (index, info) in accounts_count::ALL {
            self.types.insert(*index, (*info).clone());
            self.values.insert(
                *index,
                Arc::new(RwLock::new(DataValue::create(&info.r#type))),
            );
        }

        Ok(self)
    }

    pub fn with_type(mut self, index: DataIndex, info: &DataInfo) -> Result<Self, ManyError> {
        if let Entry::Vacant(e) = self.types.entry(index) {
            e.insert((*info).clone());
            self.values.insert(
                index,
                Arc::new(RwLock::new(DataValue::create(&info.r#type))),
            );
            Ok(self)
        } else {
            Err(ManyError::unknown("Type already registered."))
        }
    }

    pub fn register_type(
        &mut self,
        index: DataIndex,
        info: DataInfo,
    ) -> Result<Arc<RwLock<DataValue>>, ManyError> {
        if self.types.contains_key(&index) {
            return Err(ManyError::unknown("Type already registered."));
        }

        let v = Arc::new(RwLock::new(DataValue::create(&info.r#type)));
        self.types.insert(index, info);
        self.values.insert(index, v.clone());
        Ok(v)
    }

    fn decode_inner(
        &mut self,
        d: &mut Decoder,
        skip: bool,
    ) -> Result<BTreeMap<DataIndex, DataValue>, Box<dyn std::error::Error>> {
        let mut len = d.map().map_err(ManyError::deserialization_error)?;
        let mut new_values = BTreeMap::new();

        loop {
            if let Some(ref mut l) = len {
                if *l == 0 {
                    break;
                }
                *l -= 1;
            } else if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            let index: DataIndex = d.decode()?;
            if self.type_of(index).is_none() {
                if skip {
                    continue;
                } else {
                    return Err(Box::new(ManyError::unknown("Unknown data type.")));
                }
            }

            let value = d.decode()?;
            new_values.insert(index, value);
        }

        Ok(new_values)
    }

    /// Decode a CBOR map and merge its output with this data set, replacing
    /// any existing data on conflict.
    /// If the `skip` argument is true, any unknown data types will be skipped
    /// instead of returning an error.
    pub fn decode_merge(&mut self, d: &mut Decoder, skip: bool) -> Result<(), ManyError> {
        let map = self
            .decode_inner(d, skip)
            .map_err(ManyError::deserialization_error)?;

        self.merge(map)
    }

    pub fn merge(
        &mut self,
        it: impl IntoIterator<Item = (DataIndex, DataValue)>,
    ) -> Result<(), ManyError> {
        for (index, v) in it.into_iter() {
            if let Some(lock) = self.values.get(&index) {
                let mut guard = lock.write().map_err(ManyError::deserialization_error)?;
                guard.set(v)?;
            }
        }
        Ok(())
    }

    /// Encode a CBOR map from a selected source of indices.
    pub fn encode_indices<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        indices: impl IntoIterator<Item = DataIndex>,
    ) -> Result<(), encode::Error<W::Error>> {
        let mut set: BTreeSet<DataIndex> = indices.into_iter().collect();
        e.encode_with(self, &mut set)?;
        Ok(())
    }

    pub fn type_of(&self, index: DataIndex) -> Option<&DataType> {
        self.types.get(&index).map(|i| &i.r#type)
    }

    pub fn info(&self, index: DataIndex) -> Option<&DataInfo> {
        self.types.get(&index)
    }

    pub fn infos(&self) -> std::collections::btree_map::Iter<'_, DataIndex, DataInfo> {
        self.types.iter()
    }

    /// Get multiple cloned values from a list of indices. If an indices in the list isn't in the
    /// set, it will not contain a value on the output BTreeMap.
    /// If any lock is poisoned, this function will return the first poisoned error.
    pub fn get_multiple(
        &mut self,
        indices: impl IntoIterator<Item = DataIndex>,
    ) -> Result<BTreeMap<DataIndex, DataValue>, PoisonError<RwLockReadGuard<'_, DataValue>>> {
        let set: BTreeSet<DataIndex> = indices.into_iter().collect();
        self.values
            .iter()
            .filter(|(k, _v)| set.contains(k))
            .map(|(k, v)| Ok((*k, v.read().map(|v| v.clone())?)))
            .collect()
    }

    /// Returns a _copy_ of an internal value.
    pub fn get_value(&self, index: DataIndex) -> Option<DataValue> {
        self.values.get(&index)?.read().ok().map(|x| x.clone())
    }

    pub fn get(
        &self,
        index: DataIndex,
    ) -> Option<Result<DataValue, PoisonError<RwLockReadGuard<'_, DataValue>>>> {
        self.values.get(&index).map(|v| {
            let value = v.read()?;
            Ok(value.clone())
        })
    }

    pub fn get_mut(&mut self, index: DataIndex) -> Option<Arc<RwLock<DataValue>>> {
        self.values.get(&index).map(Clone::clone)
    }

    pub fn delete(&mut self, index: DataIndex) {
        self.values.remove(&index);
    }
}

impl Encode<()> for DataSet {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut (),
    ) -> Result<(), encode::Error<W::Error>> {
        e.map(self.values.len() as u64)?;

        for (k, v) in &self.values {
            let v = v.read().unwrap_or_else(|x| x.into_inner()).clone();
            e.encode(k)?.encode(v)?;
        }
        Ok(())
    }
}

impl Encode<BTreeSet<DataIndex>> for DataSet {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        ctx: &mut BTreeSet<DataIndex>,
    ) -> Result<(), encode::Error<W::Error>> {
        let it = self.values.iter().filter(|(k, _)| ctx.contains(*k));
        let count = it.clone().count();
        e.map(count as u64)?;

        for (k, v) in it {
            let v = v.read().unwrap_or_else(|x| x.into_inner()).clone();
            e.encode(k)?.encode(v)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataValue {
    Counter(u64),
    GaugeInt(BigInt),
}

impl DataValue {
    pub fn create(ty: &DataType) -> Self {
        match ty {
            DataType::Counter => Self::Counter(0),
            DataType::GaugeInt => Self::GaugeInt(BigInt::default()),
        }
    }

    pub fn set(&mut self, new: DataValue) -> Result<(), ManyError> {
        match (self, new) {
            (DataValue::Counter(inner), DataValue::Counter(x)) => *inner = x,
            (DataValue::GaugeInt(inner), DataValue::GaugeInt(x)) => *inner = x,
            _ => return Err(ManyError::unknown("Incompatible Data types.")),
        }

        Ok(())
    }

    pub fn as_counter(&self) -> Option<u64> {
        match self {
            Self::Counter(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_gauge_int(&self) -> Option<&BigInt> {
        match self {
            Self::GaugeInt(v) => Some(v),
            _ => None,
        }
    }
}

impl<C> Encode<C> for DataValue {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        match self {
            DataValue::Counter(v) => {
                e.tag(Tag::Unassigned(DataType::Counter as u64))?.u64(*v)?;
            }
            DataValue::GaugeInt(v) => {
                e.tag(Tag::Unassigned(DataType::GaugeInt as u64))?;
                if let Some(v) = v.to_i64() {
                    e.i64(v)?;
                } else {
                    match v.to_bytes_be() {
                        (Sign::Plus, bytes) => {
                            e.tag(Tag::PosBignum)?.bytes(&bytes)?;
                        }
                        (Sign::Minus, bytes) => {
                            e.tag(Tag::NegBignum)?.bytes(&bytes)?;
                        }
                        (Sign::NoSign, _) => {
                            e.tag(Tag::PosBignum)?.bytes(&[])?;
                        }
                    };
                }
            }
        }

        Ok(())
    }
}

impl<'b> Decode<'b, ()> for DataValue {
    fn decode(d: &mut Decoder<'b>, _: &mut ()) -> Result<Self, decode::Error> {
        match d.tag()? {
            Tag::Unassigned(10100) => Ok(Self::Counter(d.u64()?)),
            Tag::Unassigned(10101) => match d.datatype()? {
                Type::U8 | Type::U16 | Type::U32 | Type::U64 => Ok(Self::GaugeInt(d.u64()?.into())),
                Type::Tag => {
                    let bigint = match d.tag()? {
                        Tag::PosBignum => BigInt::from_bytes_be(Sign::Plus, d.bytes()?),
                        Tag::NegBignum => BigInt::from_bytes_be(Sign::Minus, d.bytes()?),
                        _ => {
                            return Err(decode::Error::message("Unsupported tag for big numbers."))
                        }
                    };
                    Ok(Self::GaugeInt(bigint))
                }
                _ => Err(decode::Error::type_mismatch(Type::U64)),
            },
            _ => Err(decode::Error::message("Unsupported tag for big numbers.")),
        }
    }
}

impl From<DataValue> for BigInt {
    fn from(value: DataValue) -> Self {
        match value {
            DataValue::Counter(v) => v.into(),
            DataValue::GaugeInt(v) => v,
        }
    }
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
pub struct DataInfo {
    #[n(0)]
    pub r#type: DataType,
    #[n(1)]
    pub shortname: Cow<'static, str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::accounts_count::{TOTAL_COUNT_INDEX, TOTAL_COUNT_INFO};

    #[test]
    fn supports_bigint() {
        use num_traits::Num;

        let query_return = cbor_diag::parse_diag(
            r#"{ [0, [2, 0]]: 10101(2(h'0102030405060708090A0B0C0D0E0F')) }"#,
        )
        .unwrap()
        .to_bytes();

        let mut ds = DataSet::default()
            .with_type(TOTAL_COUNT_INDEX, &TOTAL_COUNT_INFO)
            .unwrap();
        ds.decode_merge(&mut Decoder::new(&query_return), false)
            .unwrap();

        assert_eq!(
            ds.get(TOTAL_COUNT_INDEX)
                .unwrap()
                .unwrap()
                .as_gauge_int()
                .unwrap(),
            &BigInt::from_str_radix("0102030405060708090A0B0C0D0E0F", 16).unwrap()
        );

        let bytes = minicbor::to_vec(&ds).unwrap();
        assert_eq!(&query_return, &bytes);
    }

    #[test]
    fn supports_u64() {
        let query_return = cbor_diag::parse_diag(r#"{ [0, [2, 0]]: 10101(66051) }"#)
            .unwrap()
            .to_bytes();

        let mut ds = DataSet::default()
            .with_type(TOTAL_COUNT_INDEX, &TOTAL_COUNT_INFO)
            .unwrap();
        ds.decode_merge(&mut Decoder::new(&query_return), false)
            .unwrap();

        assert_eq!(
            ds.get(TOTAL_COUNT_INDEX)
                .unwrap()
                .unwrap()
                .as_gauge_int()
                .unwrap()
                .to_u64()
                .unwrap(),
            0x010203
        );

        let bytes = minicbor::to_vec(&ds).unwrap();
        assert_eq!(&query_return, &bytes);
    }

    #[test]
    fn fails_on_unknown_index() {
        let query_return = cbor_diag::parse_diag(r#"{ [0, [2, 3]]: 66051 }"#)
            .unwrap()
            .to_bytes();

        let mut ds = DataSet::default().with_known_types().unwrap();

        assert!(ds
            .decode_merge(&mut Decoder::new(&query_return), false)
            .is_err());
    }

    #[test]
    fn fails_on_invalid_data_type() {
        let query_return = cbor_diag::parse_diag(r#"{ [0, [2, 0]]: "a" }"#)
            .unwrap()
            .to_bytes();

        let mut ds = DataSet::default().with_known_types().unwrap();
        let err = ds
            .decode_merge(&mut Decoder::new(&query_return), false)
            .unwrap_err();
        assert_eq!(err.code(), many_error::ManyErrorCode::DeserializationError);
    }

    #[test]
    fn encodes_selectively() {
        let query_return = cbor_diag::parse_diag(r#"{ [0, [2, 0]]: 10101(66051) }"#)
            .unwrap()
            .to_bytes();

        let mut ds = DataSet::default().with_known_types().unwrap();
        ds.decode_merge(&mut Decoder::new(&query_return), false)
            .unwrap();

        assert_eq!(
            ds.get(TOTAL_COUNT_INDEX)
                .unwrap()
                .unwrap()
                .as_gauge_int()
                .unwrap()
                .to_u64()
                .unwrap(),
            0x010203
        );

        // This should be non-equal because it contains types that aren't in the
        // original buffer.
        let bytes = minicbor::to_vec(&ds).unwrap();
        assert_ne!(&query_return, &bytes);

        // Let's try again with the set of indices we have in the input buffer.
        let bytes = minicbor::to_vec_with(&ds, &mut BTreeSet::from([TOTAL_COUNT_INDEX])).unwrap();
        assert_eq!(&query_return, &bytes);

        // Let's try again with an unknown index.
        let bytes = minicbor::to_vec_with(&ds, &mut BTreeSet::from([DataIndex::new(1)])).unwrap();
        assert_eq!(&vec![160], &bytes); // Empty map.
    }
}
