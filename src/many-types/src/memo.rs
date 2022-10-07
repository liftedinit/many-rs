use crate::Either;
use many_error::ManyError;
use minicbor::bytes::ByteVec;
use minicbor::data::Type;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};

const MEMO_DATA_DEFAULT_MAX_SIZE: usize = 4000; // 4kB

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
enum MemoInner<const MAX_LENGTH: usize> {
    String(String),
    ByteString(ByteVec),
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

/// A memo contains a human readable portion and/or a machine readable portion.
/// It is meant to be a note regarding a message, transaction, info or any
/// type that requires meta information.
#[derive(Clone, Debug, PartialOrd, Eq, PartialEq)]
pub struct Memo<const MAX_LENGTH: usize = MEMO_DATA_DEFAULT_MAX_SIZE> {
    /// This has an invariant that the vector should never be empty. This is verified by being
    /// impossible to create an empty memo using methods or `From`/`TryFrom`s, and also during
    /// decoding of the Memo.
    inner: Vec<MemoInner<MAX_LENGTH>>,
}

impl<const M: usize> Memo<M> {
    /// Adds a string at the end.
    pub fn push_str(&mut self, str: String) -> Result<(), ManyError> {
        self.inner.push(MemoInner::<M>::try_from(str)?);
        Ok(())
    }

    pub fn push_byte_vec(&mut self, bytes: ByteVec) -> Result<(), ManyError> {
        self.inner.push(MemoInner::<M>::try_from(bytes)?);
        Ok(())
    }

    /// Returns an iterator over all strings of the memo.
    pub fn iter_str(&self) -> impl Iterator<Item = &String> {
        self.inner.iter().filter_map(|inner| match inner {
            MemoInner::String(s) => Some(s),
            MemoInner::ByteString(_) => None,
        })
    }

    /// Returns an iterator over all bytestrings of the memo.
    pub fn iter_bytes(&self) -> impl Iterator<Item = &[u8]> {
        self.inner.iter().filter_map(|inner| match inner {
            MemoInner::String(_) => None,
            MemoInner::ByteString(bstr) => Some(bstr.as_slice()),
        })
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
        // Allow for backward compatibility when using a feature.
        // We need this if we move a database with existing memos.
        #[cfg(feature = "memo-backward-compatible")]
        match d.datatype()? {
            Type::Bytes => {
                return Self::try_from(d.bytes()?.to_vec()).map_err(decode::Error::message);
            }
            Type::String => {
                return Self::try_from(d.str()?.to_string()).map_err(decode::Error::message);
            }
            _ => {}
        }

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

#[cfg(test)]
mod tests {
    use super::{Memo, MEMO_DATA_DEFAULT_MAX_SIZE};
    use proptest::proptest;

    proptest! {
        #[test]
        fn memo_str_decode_prop(len in 900..1100usize) {
            let data = String::from_utf8(vec![b'A'; len]).unwrap();
            let cbor = format!(r#" [ "{data}" ] "#);
            let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

            let result = minicbor::decode::<Memo<1000>>(&bytes);
            if len <= 1000 {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
            }
        }

        #[test]
        fn memo_bytes_decode_prop(len in 900..1100usize) {
            let data = hex::encode(vec![1u8; len]);
            let cbor = format!(r#" [ h'{data}' ] "#);
            let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

            let result = minicbor::decode::<Memo<1000>>(&bytes);
            if len <= 1000 {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn memo_decode_ok() {
        let data = String::from_utf8(vec![b'A'; MEMO_DATA_DEFAULT_MAX_SIZE]).unwrap();
        let cbor = format!(r#" [ "{data}" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_ok());
    }

    #[test]
    fn memo_decode_too_large() {
        let data = String::from_utf8(vec![b'A'; MEMO_DATA_DEFAULT_MAX_SIZE + 1]).unwrap();
        let cbor = format!(r#" [ "{data}" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }

    #[test]
    fn memo_decode_empty() {
        let cbor = " [] ";
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }

    #[test]
    fn data_decode_ok() {
        let data = hex::encode(vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE]);
        let cbor = format!(r#" [ h'{data}' ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_ok());
    }

    #[test]
    fn data_decode_large() {
        let data = hex::encode(vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE + 1]);
        let cbor = format!(r#" [ h'{data}' ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }

    #[test]
    fn mixed_decode_ok() {
        let data = hex::encode(vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE]);
        let cbor = format!(r#" [ "", h'{data}', "" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_ok());
    }

    #[test]
    fn mixed_decode_data_too_lare() {
        let data = hex::encode(vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE + 1]);
        let cbor = format!(r#" [ "", h'{data}', "" ] "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }

    #[test]
    fn mixed_decode_data_type_mismatch() {
        let cbor = r#" 0 "#;
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }

    #[test]
    fn mixed_decode_data_type_mismatch_array() {
        let cbor = r#" [ "", 0, "" ] "#;
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        assert!(minicbor::decode::<Memo>(&bytes).is_err());
    }

    #[test]
    fn backward_compatibility_str() {
        let data = String::from_utf8(vec![b'A'; MEMO_DATA_DEFAULT_MAX_SIZE]).unwrap();
        let cbor = format!(r#" "{data}" "#);
        let bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let memo = minicbor::decode::<Memo>(&bytes).unwrap();
        assert_eq!(memo.iter_str().next(), Some(&data));
    }

    #[test]
    fn backward_compatibility_bytes() {
        let bytes = vec![1u8; MEMO_DATA_DEFAULT_MAX_SIZE];
        let data = hex::encode(&bytes);
        let cbor = format!(r#" h'{data}' "#);
        let cbor_bytes = cbor_diag::parse_diag(cbor).unwrap().to_bytes();

        let memo = minicbor::decode::<Memo>(&cbor_bytes).unwrap();
        assert_eq!(memo.iter_bytes().next(), Some(bytes.as_slice()));
    }
}
