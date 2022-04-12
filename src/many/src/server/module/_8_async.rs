use crate::{Identity, ManyError};
use many_macros::many_module;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use minicose::CoseSign1;

/// An AsyncToken which is returned when the server does not have an immediate
/// response.
#[derive(Clone)]
#[repr(transparent)]
pub struct AsyncToken(Vec<u8>);

impl AsRef<[u8]> for AsyncToken {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Encode for AsyncToken {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.bytes(&self.0)?;
        Ok(())
    }
}

impl<'b> Decode<'b> for AsyncToken {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
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
    use super::AsyncToken;
    use crate::cbor::CborAny;
    use crate::protocol::attributes::TryFromAttributeSet;
    use crate::protocol::{Attribute, AttributeSet};
    use crate::ManyError;

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
                return Err(ManyError::invalid_attribute_id(value.id.to_string()));
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

#[derive(Debug, Clone, Encode, Decode)]
#[cbor(map)]
pub struct StatusArgs {
    #[n(0)]
    pub token: AsyncToken,
}

pub enum RequestStatus {
    Unknown = 0,
    Processing = 1,
    Done = 2,
}

#[derive(Debug, Clone)]
pub enum StatusReturn {
    Unknown,
    Queued,
    Processing,
    Done { response: CoseSign1 },
    Expired,
}

impl Encode for StatusReturn {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        match self {
            StatusReturn::Unknown => e.map(1)?.u8(0)?.u8(0)?,
            StatusReturn::Queued => e.map(1)?.u8(0)?.u8(1)?,
            StatusReturn::Processing => e.map(1)?.u8(0)?.u8(2)?,
            StatusReturn::Done { response } => {
                let bytes = response
                    .to_bytes()
                    .map_err(|_err| Error::Message("Response could not be encoded."))?;
                e.map(2)?.u8(0)?.u8(3)?.u8(1)?.bytes(&bytes)?
            }
            StatusReturn::Expired => e.map(1)?.u8(0)?.u8(4)?,
        };

        Ok(())
    }
}

impl<'b> Decode<'b> for StatusReturn {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
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
                    x => return Err(minicbor::decode::Error::UnknownVariant(x as u32)),
                },

                _ => return Err(minicbor::decode::Error::Message("Invalid key type.")),
            }

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        match (key, result) {
            (Some(0), None) => Ok(Self::Unknown),
            (Some(1), None) => Ok(Self::Queued),
            (Some(2), None) => Ok(Self::Processing),
            (Some(3), Some(result)) => Ok(Self::Done {
                response: CoseSign1::from_bytes(result).map_err(|_| {
                    minicbor::decode::Error::Message("Invalid result type, expected CoseSign1.")
                })?,
            }),
            (Some(4), None) => Ok(Self::Expired),
            _ => Err(minicbor::decode::Error::Message(
                "Invalid variant or result.",
            )),
        }
    }
}

#[many_module(name = AsyncModule, id = 8, namespace = async, many_crate = crate)]
pub trait AsyncModuleBackend: Send {
    fn status(&mut self, sender: &Identity, args: StatusArgs) -> Result<StatusReturn, ManyError>;
}
