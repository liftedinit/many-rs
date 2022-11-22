use crate::Either;
use many_error::ManyError;
use minicbor::bytes::ByteVec;
use minicbor::data::Type;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use std::borrow::Cow;

const MEMO_DATA_DEFAULT_MAX_SIZE: usize = 4000; // 4kB

mod legacy;
pub use legacy::Data as DataLegacy;
pub use legacy::Memo as MemoLegacy;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
enum MemoInner<const MAX_LENGTH: usize> {
    String(String),
    ByteString(ByteVec),
}

impl<const M: usize> MemoInner<M> {
    pub fn as_string(&self) -> Option<&String> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
}

macro_rules! declare_try_from {
    ( $( $ty: ty = $item: path );* $(;)? ) => {
        $(
        impl<const M: usize> TryFrom<$ty> for MemoInner<M> {
            type Error = ManyError;

            fn try_from(value: $ty) -> Result<Self, Self::Error> {
                if value.len() > M {
                    return Err(ManyError::unknown(format!(
                        "Data size ({}) over limit ({})",
                        value.len(),
                        M
                    )));
                }
                Ok($item(value.into()))
            }
        }
        )*
    };
}

declare_try_from!(
    String = Self::String;
    &str = Self::String;
    Cow<'_, str> = Self::String;
    ByteVec = Self::ByteString;
    Vec<u8> = Self::ByteString;
);

impl<const M: usize> TryFrom<Either<String, ByteVec>> for MemoInner<M> {
    type Error = ManyError;

    fn try_from(value: Either<String, ByteVec>) -> Result<Self, Self::Error> {
        match value {
            Either::Left(str) => Self::try_from(str),
            Either::Right(bstr) => Self::try_from(bstr),
        }
    }
}

impl<const M: usize> TryFrom<Either<String, Vec<u8>>> for MemoInner<M> {
    type Error = ManyError;

    fn try_from(value: Either<String, Vec<u8>>) -> Result<Self, Self::Error> {
        match value {
            Either::Left(str) => Self::try_from(str),
            Either::Right(bstr) => Self::try_from(ByteVec::from(bstr)),
        }
    }
}

impl<C, const M: usize> Encode<C> for MemoInner<M> {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        match self {
            MemoInner::String(str) => e.str(str),
            MemoInner::ByteString(bstr) => e.bytes(bstr.as_slice()),
        }
        .map(|_| ())
    }
}

impl<'b, C, const M: usize> Decode<'b, C> for MemoInner<M> {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        match d.datatype()? {
            Type::Bytes => Self::try_from(d.bytes()?.to_vec()).map_err(decode::Error::message),
            Type::String => Self::try_from(d.str()?).map_err(decode::Error::message),
            // Type::BytesIndef => {}
            // Type::StringIndef => {}
            _ => Err(decode::Error::type_mismatch(Type::String)),
        }
    }
}

/// A memo contains a human-readable portion and/or a machine readable portion.
/// It is meant to be a note regarding a message, transaction, info or any
/// type that requires meta information.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Memo<const MAX_LENGTH: usize = MEMO_DATA_DEFAULT_MAX_SIZE> {
    /// This has an invariant that the vector should never be empty. This is verified by being
    /// impossible to create an empty memo using methods or `From`/`TryFrom`s, and also during
    /// decoding of the Memo.
    inner: Vec<MemoInner<MAX_LENGTH>>,
}

impl<const M: usize> Memo<M> {
    pub fn try_from_iter(
        iter: impl IntoIterator<Item = impl Into<Either<String, Vec<u8>>>>,
    ) -> Result<Self, ManyError> {
        let inner = iter
            .into_iter()
            .map(|item| {
                let either: Either<String, Vec<u8>> = item.into();
                either.try_into()
            })
            .collect::<Result<_, ManyError>>()?;
        Ok(Self { inner })
    }

    /// Adds a string at the end.
    pub fn push_str<'a>(&mut self, str: impl Into<Cow<'a, str>>) -> Result<(), ManyError> {
        self.inner
            .push(MemoInner::<M>::try_from(str.into().into_owned())?);
        Ok(())
    }

    pub fn push_bytes<'a>(&mut self, bytes: impl Into<Cow<'a, [u8]>>) -> Result<(), ManyError> {
        let bytes = bytes.into();
        self.inner
            .push(MemoInner::<M>::try_from(ByteVec::from(bytes.to_vec()))?);
        Ok(())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns an iterator over all strings of the memo.
    pub fn iter_str(&self) -> impl Iterator<Item = &String> {
        self.inner.iter().filter_map(MemoInner::as_string)
    }

    /// Returns an iterator over all bytestrings of the memo.
    pub fn iter_bytes(&self) -> impl Iterator<Item = &[u8]> {
        self.inner.iter().filter_map(|inner| match inner {
            MemoInner::String(_) => None,
            MemoInner::ByteString(bstr) => Some(bstr.as_slice()),
        })
    }
}

// This helps comparisons.
impl<const M: usize> PartialEq<str> for Memo<M> {
    fn eq(&self, other: &str) -> bool {
        if self.len() == 1 {
            return self.iter_str().next().map(String::as_str) == Some(other);
        }

        false
    }
}

impl<const M: usize> PartialEq<&str> for Memo<M> {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl<const M: usize> From<MemoInner<M>> for Memo<M> {
    fn from(inner: MemoInner<M>) -> Self {
        Self { inner: vec![inner] }
    }
}

impl<const M: usize> TryFrom<Either<String, ByteVec>> for Memo<M> {
    type Error = ManyError;

    fn try_from(s: Either<String, ByteVec>) -> Result<Self, Self::Error> {
        Ok(Self::from(MemoInner::<M>::try_from(s)?))
    }
}

impl<const M: usize> TryFrom<String> for Memo<M> {
    type Error = ManyError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(Self::from(MemoInner::<M>::try_from(s)?))
    }
}

impl<'a, const M: usize> TryFrom<Cow<'a, str>> for Memo<M> {
    type Error = ManyError;
    fn try_from(s: Cow<'a, str>) -> Result<Self, Self::Error> {
        Ok(Self::from(MemoInner::<M>::try_from(s)?))
    }
}

impl<const M: usize> TryFrom<&str> for Memo<M> {
    type Error = ManyError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(Self::from(MemoInner::<M>::try_from(s)?))
    }
}

impl<const M: usize> TryFrom<ByteVec> for Memo<M> {
    type Error = ManyError;
    fn try_from(b: ByteVec) -> Result<Self, Self::Error> {
        Ok(Self::from(MemoInner::<M>::try_from(b)?))
    }
}

impl<const M: usize> TryFrom<Vec<u8>> for Memo<M> {
    type Error = ManyError;
    fn try_from(b: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(Self::from(MemoInner::<M>::try_from(b)?))
    }
}

impl<C, const M: usize> Encode<C> for Memo<M> {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.encode(&self.inner).map(|_| ())
    }
}

impl<'b, C, const M: usize> Decode<'b, C> for Memo<M> {
    fn decode(d: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, decode::Error> {
        let inner = d
            .array_iter_with(ctx)?
            .collect::<Result<Vec<MemoInner<M>>, _>>()?;
        if inner.is_empty() {
            Err(decode::Error::message("Cannot build empty Memo."))
        } else {
            Ok(Self { inner })
        }
    }
}

// Moving from Legacy to new type can be done safely if the limit is the same.
impl<S: AsRef<str>> From<MemoLegacy<S>> for Memo<MEMO_DATA_DEFAULT_MAX_SIZE> {
    fn from(value: MemoLegacy<S>) -> Self {
        Self::from(MemoInner::try_from(value.as_ref()).unwrap())
    }
}

// Moving from Legacy to new type can be done safely if the limit is the same.
impl From<DataLegacy> for Memo<MEMO_DATA_DEFAULT_MAX_SIZE> {
    fn from(value: DataLegacy) -> Self {
        Self::from(MemoInner::try_from(value.0).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::proptest;

    proptest! {
        #[test]
        fn memo_str_decode_prop(len in 900..1100usize) {
            let data = String::from_utf8(vec![b'A'; len]).unwrap();
            let cbor = format!(r#" [ "{data}" ] "#);
            let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

            let result = minicbor::decode::<Memo<1000>>(&bytes);
            if len <= 1000 {
                let result = result.unwrap();
                let mut it_str = result.iter_str();
                assert_eq!(it_str.next(), Some(&data));
                assert_eq!(it_str.next(), None);
                let mut it_bytes = result.iter_bytes();
                assert_eq!(it_bytes.next(), None);
            } else {
                let err = result.unwrap_err();
                assert!(err.to_string().contains(&format!("Data size ({len}) over limit (1000)")))
            }
        }

        #[test]
        fn memo_bytes_decode_prop(len in 900..1100usize) {
            let data_raw = vec![1u8; len];
            let data = hex::encode(&data_raw);
            let cbor = format!(r#" [ h'{data}' ] "#);
            let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

            let result = minicbor::decode::<Memo<1000>>(&bytes);
            if len <= 1000 {
                let result = result.unwrap();
                let mut it_str = result.iter_str();
                assert_eq!(it_str.next(), None);
                let mut it_bytes = result.iter_bytes();
                assert_eq!(it_bytes.next(), Some(data_raw.as_slice()));
                assert_eq!(it_bytes.next(), None);
            } else {
                let err = result.unwrap_err();
                assert!(err.to_string().contains(&format!("Data size ({len}) over limit (1000)")))
            }
        }
    }

    #[test]
    fn memo_decode_ok() {
        let data = String::from_utf8(vec![b'A'; MEMO_DATA_DEFAULT_MAX_SIZE]).unwrap();
        let cbor = format!(r#" [ "{data}" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let memo = minicbor::decode::<Memo>(&bytes).unwrap();
        let mut it_str = memo.iter_str();
        let mut it_bytes = memo.iter_bytes();
        assert_eq!(it_str.next(), Some(&data));
        assert_eq!(it_str.next(), None);
        assert_eq!(it_bytes.next(), None);
    }

    #[test]
    fn memo_decode_too_large() {
        let data = String::from_utf8(vec![b'A'; MEMO_DATA_DEFAULT_MAX_SIZE + 1]).unwrap();
        let cbor = format!(r#" [ "{data}" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let result = minicbor::decode::<Memo>(&bytes);
        assert!(result.unwrap_err().to_string().contains(&format!(
            "Data size ({}) over limit ({MEMO_DATA_DEFAULT_MAX_SIZE})",
            MEMO_DATA_DEFAULT_MAX_SIZE + 1
        )))
    }

    #[test]
    fn memo_decode_empty() {
        let cbor = " [] ";
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let result = minicbor::decode::<Memo>(&bytes);
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot build empty Memo"))
    }

    #[test]
    fn bytes_decode_ok() {
        let data_raw = vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE];
        let data = hex::encode(&data_raw);
        let cbor = format!(r#" [ h'{data}' ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let memo = minicbor::decode::<Memo>(&bytes).unwrap();
        let mut it_str = memo.iter_str();
        let mut it_bytes = memo.iter_bytes();
        assert_eq!(it_str.next(), None);
        assert_eq!(it_bytes.next(), Some(data_raw.as_slice()));
        assert_eq!(it_bytes.next(), None);
    }

    #[test]
    fn bytes_decode_large() {
        let data = hex::encode(vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE + 1]);
        let cbor = format!(r#" [ h'{data}' ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let result = minicbor::decode::<Memo>(&bytes);
        assert!(result.unwrap_err().to_string().contains(&format!(
            "Data size ({}) over limit ({MEMO_DATA_DEFAULT_MAX_SIZE})",
            MEMO_DATA_DEFAULT_MAX_SIZE + 1
        )))
    }

    #[test]
    fn mixed_decode_ok() {
        let data_raw = vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE];
        let data = hex::encode(&data_raw);
        let cbor = format!(r#" [ "", h'{data}', "" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let memo = minicbor::decode::<Memo>(&bytes).unwrap();
        let mut it_str = memo.iter_str();
        let mut it_bytes = memo.iter_bytes();
        assert_eq!(it_str.next(), Some(&String::new()));
        assert_eq!(it_str.next(), Some(&String::new()));
        assert_eq!(it_str.next(), None);
        assert_eq!(it_bytes.next(), Some(data_raw.as_slice()));
        assert_eq!(it_bytes.next(), None);
    }

    #[test]
    fn mixed_decode_bytes_too_large() {
        let data = hex::encode(vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE + 1]);
        let cbor = format!(r#" [ "", h'{data}', "" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let result = minicbor::decode::<Memo>(&bytes);
        assert!(result.unwrap_err().to_string().contains(&format!(
            "Data size ({}) over limit ({MEMO_DATA_DEFAULT_MAX_SIZE})",
            MEMO_DATA_DEFAULT_MAX_SIZE + 1
        )))
    }

    #[test]
    fn mixed_decode_type_mismatch() {
        let cbor = r#" 0 "#;
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let result = minicbor::decode::<Memo>(&bytes);
        assert_eq!(
            result.unwrap_err().to_string(),
            "unexpected type u8 at position 0: expected array"
        );
    }

    #[test]
    fn mixed_decode_type_mismatch_array() {
        let cbor = r#" [ "", 0, "" ] "#;
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let result = minicbor::decode::<Memo>(&bytes);
        assert_eq!(result.unwrap_err().to_string(), "unexpected type string");
    }

    #[test]
    fn backward_compatibility_str() {
        let data = String::from_utf8(vec![b'A'; MEMO_DATA_DEFAULT_MAX_SIZE]).unwrap();
        let cbor = format!(r#" "{data}" "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let memo: Memo = minicbor::decode::<MemoLegacy<String>>(&bytes)
            .unwrap()
            .into();
        let mut it_str = memo.iter_str();
        assert_eq!(it_str.next(), Some(&data));
        assert_eq!(it_str.next(), None);
    }

    #[test]
    fn backward_compatibility_bytes() {
        let bytes = vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE];
        let data = hex::encode(&bytes);
        let cbor = format!(r#" h'{data}' "#);
        let cbor_bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let data_legacy = minicbor::decode::<DataLegacy>(&cbor_bytes).unwrap();
        let memo: Memo = data_legacy.into();
        let mut it = memo.iter_bytes();
        assert_eq!(it.next(), Some(bytes.as_slice()));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn backward_compatibility_empty_str() {
        let data = String::new();
        let cbor = format!(r#" "{data}" "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let memo_legacy = minicbor::decode::<MemoLegacy<String>>(&bytes).unwrap();
        let memo: Memo = memo_legacy.into();
        let mut it = memo.iter_str();
        assert_eq!(it.next(), Some(&data));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn backward_compatibility_empty_bytes() {
        let bytes = Vec::new();
        let data = hex::encode(&bytes);
        let cbor = format!(r#" h'{data}' "#);
        let cbor_bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let memo: Memo = minicbor::decode::<DataLegacy>(&cbor_bytes).unwrap().into();
        assert_eq!(memo.iter_bytes().next(), Some(bytes.as_slice()));
    }

    #[test]
    fn mixed_iterators() {
        let cbor = r#" [ "1", h'02', "3", h'04', h'05', "6", "7", "8", h'09' ] "#.to_string();
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let memo = minicbor::decode::<Memo>(&bytes).unwrap();
        assert_eq!(
            memo.iter_str().collect::<Vec<&String>>(),
            &["1", "3", "6", "7", "8"]
        );
        assert_eq!(
            memo.iter_bytes().map(hex::encode).collect::<Vec<String>>(),
            &["02", "04", "05", "09"]
        );
    }

    #[test]
    fn memo_mut() {
        let mut memo: Memo = Memo::try_from("Hello World".to_string()).unwrap();
        assert_eq!(memo.len(), 1);
        memo.push_str("Hello Other").unwrap();
        assert_eq!(memo.len(), 2);
        memo.push_bytes(b"Foobar".to_vec()).unwrap();
        assert_eq!(memo.len(), 3);

        // Too long?
        assert!(memo
            .push_str(String::from_utf8(vec![b'A'; 4001]).unwrap())
            .is_err());
        assert_eq!(memo.len(), 3);
        assert!(memo.push_bytes(vec![b'A'; 4001]).is_err());
        assert_eq!(memo.len(), 3);

        assert_ne!(memo, "Hello Other");
        assert_ne!(memo, *"Hello Other");
        assert_eq!(memo.iter_str().count(), 2);
        assert_eq!(memo.iter_bytes().count(), 1);
    }
}
