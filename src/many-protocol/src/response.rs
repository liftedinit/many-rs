use crate::RequestMessage;
use derive_builder::Builder;
use many_identity::Address;
use many_types::attributes::{Attribute, AttributeSet};
use minicbor::data::{Tag, Type};
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use num_derive::{FromPrimitive, ToPrimitive};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(FromPrimitive, ToPrimitive)]
#[repr(i8)]
pub enum ResponseMessageCborKey {
    ProtocolVersion = 0,
    From,
    To,
    _Endpoint, // Unused in Response.
    Result,
    Timestamp,
    Id,
    _Nonce, // Unused in Response.
    Attributes,
}

/// A MANY message response.
#[derive(Clone, Debug, Builder)]
#[builder(setter(strip_option), default)]
pub struct ResponseMessage {
    pub version: Option<u8>,
    pub from: Address,
    pub to: Option<Address>,
    pub data: Result<Vec<u8>, super::ManyError>,

    /// An optional timestamp for this response. If [None] this will be filled
    /// with [SystemTime::now()]
    pub timestamp: Option<SystemTime>,

    pub id: Option<u64>,
    pub attributes: AttributeSet,
}

impl Default for ResponseMessage {
    fn default() -> Self {
        Self {
            version: None,
            from: Address::anonymous(),
            to: None,
            data: Ok(vec![]),
            timestamp: None,
            id: None,
            attributes: Default::default(),
        }
    }
}

impl ResponseMessage {
    pub fn from_request(
        request: &RequestMessage,
        from: &Address,
        data: Result<Vec<u8>, super::ManyError>,
    ) -> Self {
        Self {
            version: Some(1),
            from: *from,
            to: request.from, // We're sending back to the same requester.
            data,
            timestamp: None, // To be filled.
            id: request.id,
            attributes: Default::default(),
        }
    }

    pub fn error(from: &Address, id: Option<u64>, data: super::ManyError) -> Self {
        Self {
            version: Some(1),
            from: *from,
            to: None,
            data: Err(data),
            timestamp: None, // To be filled.
            id,
            attributes: Default::default(),
        }
    }

    pub fn with_attribute(mut self, attr: Attribute) -> Self {
        self.attributes.insert(attr);
        self
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        minicbor::to_vec(self).map_err(|e| format!("{}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }
}

impl<C> Encode<C> for ResponseMessage {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        e.tag(Tag::Unassigned(10002))?;
        let l = 2
            + if self.from.is_anonymous() { 0 } else { 1 }
            + if self.to.is_none() || self.to == Some(Address::anonymous()) {
                0
            } else {
                1
            }
            + if self.id.is_none() { 0 } else { 1 }
            + if self.attributes.is_empty() { 0 } else { 1 };
        e.map(l)?;

        // Skip version for this version of the protocol. This message implementation
        // only supports version 1.
        // e.i8(RequestMessageCborKey::ProtocolVersion as i8)?.u8(*v)?;

        // No need to send the anonymous identity.
        if !self.from.is_anonymous() {
            e.i8(ResponseMessageCborKey::From as i8)?
                .encode(&self.from)?;
        }

        if let Some(ref i) = self.to {
            if !i.is_anonymous() {
                e.i8(ResponseMessageCborKey::To as i8)?.encode(i)?;
            }
        }

        match &self.data {
            Ok(result) => e.i8(ResponseMessageCborKey::Result as i8)?.bytes(result)?,
            Err(error) => e.i8(ResponseMessageCborKey::Result as i8)?.encode(&error)?,
        };

        e.i8(ResponseMessageCborKey::Timestamp as i8)?;
        let timestamp = self.timestamp.unwrap_or_else(SystemTime::now);
        e.tag(minicbor::data::Tag::Timestamp)?.u64(
            timestamp
                .duration_since(UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs(),
        )?;

        if let Some(ref id) = self.id {
            e.i8(ResponseMessageCborKey::Id as i8)?.u64(*id)?;
        }

        if !self.attributes.is_empty() {
            e.i8(ResponseMessageCborKey::Attributes as i8)?
                .encode(&self.attributes)?;
        }

        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for ResponseMessage {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        if d.tag()? != Tag::Unassigned(10002) {
            return Err(minicbor::decode::Error::message(
                "Invalid tag, expected 10002 for a message.",
            ));
        };

        let mut builder = ResponseMessageBuilder::default();

        let mut i = 0;
        let x = d.map()?;
        // Since we don't know if this is a indef map or a regular map, we just loop
        // through items and break when we know the map is done.
        loop {
            if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            match num_traits::FromPrimitive::from_i64(d.i64()?) {
                Some(ResponseMessageCborKey::ProtocolVersion) => builder.version(d.decode()?),
                Some(ResponseMessageCborKey::From) => builder.from(d.decode()?),
                Some(ResponseMessageCborKey::To) => builder.to(d.decode()?),
                Some(ResponseMessageCborKey::Result) => match d.datatype()? {
                    Type::Bytes => builder.data(Ok(d.bytes()?.to_vec())),
                    Type::Map => builder.data(Err(d.decode()?)),
                    _ => &mut builder,
                },
                Some(ResponseMessageCborKey::Timestamp) => {
                    // Some logic applies.
                    let t = d.tag()?;
                    if t != minicbor::data::Tag::Timestamp {
                        return Err(minicbor::decode::Error::message("Invalid tag."));
                    }

                    let secs = d.u64()?;
                    let timestamp = std::time::UNIX_EPOCH
                        .checked_add(Duration::from_secs(secs))
                        .ok_or_else(|| {
                            minicbor::decode::Error::message(
                                "duration value can not represent system time",
                            )
                        })?;
                    builder.timestamp(timestamp)
                }
                Some(ResponseMessageCborKey::Attributes) => builder.attributes(d.decode()?),
                _ => &mut builder,
            };

            i += 1;
            if x.map_or(false, |x| i >= x) {
                break;
            }
        }

        builder
            .build()
            .map_err(|_e| minicbor::decode::Error::message("could not build"))
    }
}
