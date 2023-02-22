use crate::EmptyReturn;
use many_identity::Address;
use minicbor::bytes::ByteVec;
use minicbor::data::Type;
use minicbor::{Decode, Encode};

const KVSTORE_KEY_MAX_SIZE: usize = 248; // size is u8 but storage is in "/store/" (7 bytes long);
const KVSTORE_VALUE_MAX_SIZE: usize = 64000; // 64kB

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct PutArgs {
    #[n(0)]
    #[cbor(decode_with = "decode_key")]
    pub key: ByteVec,

    #[n(1)]
    #[cbor(decode_with = "decode_value")]
    pub value: ByteVec,

    #[n(2)]
    pub alternative_owner: Option<Address>,
}

/// Data decoder. Check if the key is less than or equal to the maximum allowed size
fn decode_key<C>(d: &mut minicbor::Decoder, _: &mut C) -> Result<ByteVec, minicbor::decode::Error> {
    match d.datatype()? {
        Type::Bytes => {
            let data = d.bytes()?;
            if data.len() > KVSTORE_KEY_MAX_SIZE {
                return Err(minicbor::decode::Error::message("Key size over limit"));
            }
            Ok(data.to_vec().into())
        }
        _ => Err(minicbor::decode::Error::message(
            "Wrong key type. Expected bytes",
        )),
    }
}

/// Data decoder. Check if the value is less than or equal to the maximum allowed size
fn decode_value<C>(
    d: &mut minicbor::Decoder,
    _: &mut C,
) -> Result<ByteVec, minicbor::decode::Error> {
    match d.datatype()? {
        Type::Bytes => {
            let data = d.bytes()?;
            if data.len() > KVSTORE_VALUE_MAX_SIZE {
                return Err(minicbor::decode::Error::message("Value size over limit"));
            }
            Ok(data.to_vec().into())
        }
        _ => Err(minicbor::decode::Error::message(
            "Wrong key type. Expected bytes",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{PutArgs, KVSTORE_KEY_MAX_SIZE, KVSTORE_VALUE_MAX_SIZE};
    use minicbor::bytes::ByteVec;

    #[test]
    fn put_key_over_limit() {
        let tx = PutArgs {
            key: ByteVec::from(vec![1u8; KVSTORE_KEY_MAX_SIZE + 1]),
            value: ByteVec::from(vec![2]),
            alternative_owner: None,
        };

        let enc = minicbor::to_vec(tx).unwrap();
        let dec = minicbor::decode::<PutArgs>(&enc);
        assert!(dec.is_err());
        assert_eq!(
            dec.unwrap_err().to_string(),
            "decode error: Key size over limit",
        );
    }

    #[test]
    fn put_value_over_limit() {
        let tx = PutArgs {
            key: ByteVec::from(vec![1]),
            value: ByteVec::from(vec![1u8; KVSTORE_VALUE_MAX_SIZE + 1]),
            alternative_owner: None,
        };

        let enc = minicbor::to_vec(tx).unwrap();
        let dec = minicbor::decode::<PutArgs>(&enc);
        assert!(dec.is_err());
        assert_eq!(
            dec.unwrap_err().to_string(),
            "decode error: Value size over limit",
        );
    }

    // Test covering issue https://github.com/liftedinit/operations/issues/21
    #[test]
    fn key_not_byte() {
        let payload = "{0: \"foo\", 1: \"bar\"}";
        let payload_cbor = cbor_diag::parse_diag(payload).unwrap().to_bytes();
        let res = minicbor::decode::<PutArgs>(&payload_cbor);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "decode error: Wrong key type. Expected bytes",
        );
    }
}

pub type PutReturn = EmptyReturn;
