use crate::message::ResponseMessage;
use crate::{Identity, ManyError};
use many_macros::many_module;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};

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

#[derive(Debug, Clone, Encode, Decode)]
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
    Done { response: Box<ResponseMessage> },
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

    fn from_kind(kind: u8, response: Option<ResponseMessage>) -> Result<Self, ()> {
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

impl Encode for StatusReturn {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        if let StatusReturn::Done { response } = self {
            e.map(2)?;
            e.u8(1)?.bytes(
                response
                    .clone()
                    .to_bytes()
                    .map_err(|_err| Error::Message("Response could not be encoded."))?
                    .as_ref(),
            )?
        } else {
            e.map(1)?
        }
        .u8(0)?
        .u8(self.variant())?;

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
                    x => return Err(minicbor::decode::Error::UnknownVariant(u32::from(x))),
                },

                _ => return Err(minicbor::decode::Error::Message("Invalid key type.")),
            }

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        Self::from_kind(
            key.map_or(Err("Invalid variant."), Ok)
                .map_err(minicbor::decode::Error::Message)?,
            match result {
                Some(result) => Some(ResponseMessage::from_bytes(result).map_err(|_| {
                    minicbor::decode::Error::Message(
                        "Invalid result type, expected ResponseMessage.",
                    )
                })?),
                _ => None,
            },
        )
        .map_err(|_| minicbor::decode::Error::Message("Invalid variant or result."))
    }
}

#[many_module(name = AsyncModule, id = 8, namespace = async, many_crate = crate)]
pub trait AsyncModuleBackend: Send {
    fn status(&self, sender: &Identity, args: StatusArgs) -> Result<StatusReturn, ManyError>;
}
