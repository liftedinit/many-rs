use crate::ResponseMessage;
use coset::{CborSerializable, CoseSign1};
use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};

#[cfg(test)]
use mockall::{automock, predicate::*};

/// An AsyncToken which is returned when the server does not have an immediate
/// response.
#[derive(Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct AsyncToken(Vec<u8>);

impl AsRef<[u8]> for AsyncToken {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<C> Encode<C> for AsyncToken {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        e.bytes(&self.0)?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for AsyncToken {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        Ok(Self(d.bytes()?.to_vec()))
    }
}

impl From<Vec<u8>> for AsyncToken {
    fn from(v: Vec<u8>) -> Self {
        Self(v)
    }
}

impl std::fmt::Debug for AsyncToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AsyncToken")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

pub mod attributes {
    use crate::r#async::AsyncToken;
    use many_error::ManyError;
    use many_types::attributes::{Attribute, AttributeSet, TryFromAttributeSet};
    use many_types::cbor::CborAny;

    pub const ASYNC: Attribute = Attribute::id(1);

    pub struct AsyncAttribute {
        pub token: AsyncToken,
    }

    impl AsyncAttribute {
        pub fn new(token: AsyncToken) -> Self {
            Self { token }
        }
    }

    impl From<AsyncAttribute> for Attribute {
        fn from(a: AsyncAttribute) -> Attribute {
            ASYNC.with_argument(CborAny::Bytes(a.token.0))
        }
    }

    impl TryFrom<Attribute> for AsyncAttribute {
        type Error = ManyError;

        fn try_from(value: Attribute) -> Result<Self, Self::Error> {
            if value.id != ASYNC.id {
                return Err(ManyError::invalid_attribute_id(value.id));
            }

            let arguments = value.into_arguments();
            if arguments.len() != 1 {
                Err(ManyError::invalid_attribute_arguments())
            } else {
                match arguments.into_iter().next() {
                    Some(CborAny::Bytes(token)) => Ok(Self {
                        token: token.into(),
                    }),
                    _ => Err(ManyError::invalid_attribute_arguments()),
                }
            }
        }
    }

    impl TryFromAttributeSet for AsyncAttribute {
        fn try_from_set(set: &AttributeSet) -> Result<Self, ManyError> {
            match set.get_attribute(ASYNC.id) {
                Some(attr) => AsyncAttribute::try_from(attr.clone()),
                None => Err(ManyError::attribute_not_found(ASYNC.id.to_string())),
            }
        }
    }
}

#[derive(Debug, Clone, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct StatusArgs {
    #[n(0)]
    pub token: AsyncToken,
}

#[derive(Debug, Clone)]
pub enum StatusReturn {
    Unknown,
    Queued,
    Processing,
    Done { response: Box<CoseSign1> },
    Expired,
}

impl StatusReturn {
    fn variant(&self) -> u8 {
        match self {
            StatusReturn::Unknown => 0,
            StatusReturn::Queued => 1,
            StatusReturn::Processing => 2,
            StatusReturn::Done { .. } => 3,
            StatusReturn::Expired => 4,
        }
    }

    fn from_kind(kind: u8, response: Option<CoseSign1>) -> Result<Self, ()> {
        match (kind, response) {
            (0, None) => Ok(Self::Unknown),
            (1, None) => Ok(Self::Queued),
            (2, None) => Ok(Self::Processing),
            (3, Some(response)) => Ok(Self::Done {
                response: Box::new(response),
            }),
            (4, None) => Ok(Self::Expired),
            _ => Err(()),
        }
    }
}

impl<C> Encode<C> for StatusReturn {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        if let StatusReturn::Done { response } = self {
            e.map(2)?;
            e.u8(1)?
                .bytes(response.clone().to_vec().map_err(Error::message)?.as_ref())?
        } else {
            e.map(1)?
        }
        .u8(0)?
        .u8(self.variant())?;

        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for StatusReturn {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        let len = d.map()?;
        let mut i = 0;

        let mut key = None;
        let mut result = None;

        loop {
            match d.datatype()? {
                Type::Break => {
                    d.skip()?;
                    break;
                }

                Type::U8 | Type::U16 | Type::U32 | Type::U64 => match d.u8()? {
                    0 => {
                        key = Some(d.u8()?);
                    }
                    1 => {
                        result = Some(d.bytes()?);
                    }
                    x => return Err(minicbor::decode::Error::unknown_variant(u32::from(x))),
                },

                _ => return Err(minicbor::decode::Error::message("Invalid key type.")),
            }

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        Self::from_kind(
            key.map_or(Err("Invalid variant."), Ok)
                .map_err(minicbor::decode::Error::message)?,
            match result {
                Some(result) => {
                    let cose =
                        CoseSign1::from_slice(result).map_err(minicbor::decode::Error::message)?;
                    let _response =
                        ResponseMessage::from_bytes(cose.payload.as_ref().ok_or_else(|| {
                            minicbor::decode::Error::message("Empty payload, expected CoseSign1.")
                        })?)
                        .map_err(|_| {
                            minicbor::decode::Error::message(
                                "Invalid envelope payload type, expected ResponseMessage.",
                            )
                        })?;
                    Some(cose)
                }
                _ => None,
            },
        )
        .map_err(|_| minicbor::decode::Error::message("Invalid variant or result."))
    }
}

#[many_module(name = AsyncModule, id = 8, namespace = async, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait AsyncModuleBackend: Send {
    fn status(&self, sender: &Address, args: StatusArgs) -> Result<StatusReturn, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::attributes::AsyncAttribute;
    use super::*;
    use crate::r#async::attributes::ASYNC;
    use crate::testutils::call_module_cbor;
    use many_identity::AnonymousIdentity;
    use many_protocol::encode_cose_sign1_from_response;
    use many_types::attributes::{Attribute, AttributeSet, TryFromAttributeSet};
    use many_types::cbor::CborAny;
    use mockall::predicate;
    use proptest::prelude::*;
    use std::sync::{Arc, Mutex};

    fn arb_status() -> impl Strategy<Value = StatusReturn> {
        prop_oneof![
            Just(StatusReturn::Unknown),
            Just(StatusReturn::Queued),
            Just(StatusReturn::Processing),
            Just(StatusReturn::Done {
                response: Box::new(
                    encode_cose_sign1_from_response(ResponseMessage::default(), &AnonymousIdentity)
                        .unwrap()
                )
            }),
            Just(StatusReturn::Expired),
        ]
        .boxed()
    }

    proptest! {
        #[test]
        fn status(status in arb_status()) {
            let data = StatusArgs {
                token: AsyncToken::from(vec![11, 12, 13])
            };
            let mut mock = MockAsyncModuleBackend::new();
            mock.expect_status()
                .with(predicate::eq(many_identity::testing::identity(1)), predicate::eq(data.clone()))
                .times(1)
                .return_const(Ok(status.clone()));
            let module = super::AsyncModule::new(Arc::new(Mutex::new(mock)));

            let status_return: StatusReturn = minicbor::decode(
                &call_module_cbor(1, &module, "async.status", minicbor::to_vec(data).unwrap()).unwrap(),
            )
            .unwrap();

            assert_eq!(status_return.variant(), status.variant());
        }
    }

    #[test]
    fn async_attr() {
        let v = vec![1, 2, 3, 4];
        let args = vec![CborAny::Bytes(v.clone())];

        // Valid async attr - new
        let token = AsyncToken::from(v);
        let async_attr = AsyncAttribute::new(token.clone());
        assert_eq!(async_attr.token.as_ref(), token.as_ref());

        // Valid attribute from async attribute
        let attr = Attribute::from(async_attr);
        assert_eq!(attr.id, ASYNC.id);
        assert_eq!(attr.arguments, args);

        // Valid async attr - try_from
        let attr = Attribute::new(1, args);
        let async_attr = AsyncAttribute::try_from(attr.clone()).unwrap();
        assert_eq!(async_attr.token.as_ref(), token.as_ref());

        // Valid async attr - try_from_set
        let attr_set = AttributeSet::from_iter(vec![attr].into_iter());
        let async_attr = AsyncAttribute::try_from_set(&attr_set).unwrap();
        assert_eq!(async_attr.token.as_ref(), token.as_ref());

        // Invalid async attr - try_from - invalid id
        let invalid_attr = Attribute::id(123);
        let async_attr = AsyncAttribute::try_from(invalid_attr.clone());
        assert!(async_attr.is_err());

        // Invalid async attr - try_from_set
        let attr_set = AttributeSet::from_iter(vec![invalid_attr].into_iter());
        let async_attr = AsyncAttribute::try_from_set(&attr_set);
        assert!(async_attr.is_err());

        // Invalid async attr - try_from - invalid arguments
        let invalid_attr = Attribute::id(1);
        let async_attr = AsyncAttribute::try_from(invalid_attr);
        assert!(async_attr.is_err());
    }
}
