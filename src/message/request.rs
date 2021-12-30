use crate::Identity;
use derive_builder::Builder;
use minicbor::data::{Tag, Type};
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use num_derive::{FromPrimitive, ToPrimitive};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(FromPrimitive, ToPrimitive)]
#[repr(i8)]
pub enum RequestMessageCborKey {
    ProtocolVersion = 0,
    From,
    To,
    Endpoint,
    Argument,
    Timestamp,
    Id,
    Nonce,
}

#[derive(Clone, Default, Builder)]
#[builder(setter(strip_option), default)]
pub struct RequestMessage {
    pub version: Option<u8>,
    pub from: Option<Identity>,
    pub to: Identity,
    pub method: String,
    pub data: Vec<u8>,
    pub timestamp: Option<SystemTime>,
    pub id: Option<u64>,
    pub nonce: Option<Vec<u8>>,
}

impl std::fmt::Debug for RequestMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let anon = Identity::anonymous();
        let data = hex::encode(&self.data);

        let mut s = f.debug_struct("RequestMessage");
        s.field("version", &self.version)
            .field("from", self.from.as_ref().unwrap_or(&anon))
            .field("to", &self.to)
            .field("method", &self.method)
            .field("data", &data);

        if let Some(timestamp) = &self.timestamp {
            s.field("timestamp", timestamp);
        }
        if let Some(id) = &self.id {
            s.field("id", id);
        }

        s.finish()
    }
}

impl RequestMessage {
    pub fn with_method(mut self, method: String) -> Self {
        self.method = method;
        self
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        minicbor::to_vec(self).map_err(|e| format!("{}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }

    pub fn from(&self) -> Identity {
        self.from.unwrap_or_default()
    }
}

impl Encode for RequestMessage {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.tag(Tag::Unassigned(10001))?;
        let l =
            2 + if self.from.is_none() || self.from == Some(Identity::anonymous()) {
                0
            } else {
                1
            } + if self.to.is_anonymous() { 0 } else { 1 }
                + if self.data.is_empty() { 0 } else { 1 }
                + if self.id.is_none() { 0 } else { 1 }
                + if self.nonce.is_none() { 0 } else { 1 };
        e.map(l)?;

        // Skip version for this version of the protocol. This message implementation
        // only supports version 1.
        // e.i8(RequestMessageCborKey::ProtocolVersion as i8)?.u8(*v)?;

        // No need to send the anonymous identity.
        if let Some(ref i) = self.from {
            if !i.is_anonymous() {
                e.i8(RequestMessageCborKey::From as i8)?.encode(&i)?;
            }
        }

        if !self.to.is_anonymous() {
            e.i8(RequestMessageCborKey::To as i8)?.encode(&self.to)?;
        }

        e.i8(RequestMessageCborKey::Endpoint as i8)?
            .encode(&self.method)?;

        if !self.data.is_empty() {
            e.i8(RequestMessageCborKey::Argument as i8)?
                .bytes(&self.data)?;
        }

        e.i8(RequestMessageCborKey::Timestamp as i8)?;
        let timestamp = self.timestamp.unwrap_or(SystemTime::now());
        e.tag(minicbor::data::Tag::Timestamp)?.u64(
            timestamp
                .duration_since(UNIX_EPOCH)
                .expect("Time flew backward")
                .as_secs(),
        )?;

        if let Some(ref id) = self.id {
            e.i8(RequestMessageCborKey::Id as i8)?.u64(*id)?;
        }

        if let Some(ref nonce) = self.nonce {
            e.i8(RequestMessageCborKey::Nonce as i8)?.bytes(nonce)?;
        }

        Ok(())
    }
}

impl<'b> Decode<'b> for RequestMessage {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        if d.tag()? != Tag::Unassigned(10001) {
            return Err(minicbor::decode::Error::Message(
                "Invalid tag, expected 10001 for a message.",
            ));
        };

        let mut builder = RequestMessageBuilder::default();

        let mut i = 0;
        let x = d.map()?;
        // Since we don't know if this is a indef map or a regular map, we just loop
        // through items and break when we know the map is done.
        loop {
            if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            match num_traits::FromPrimitive::from_i8(d.i8()?) {
                None => &mut builder,
                Some(RequestMessageCborKey::ProtocolVersion) => builder.version(d.decode()?),
                Some(RequestMessageCborKey::From) => builder.from(d.decode()?),
                Some(RequestMessageCborKey::To) => builder.to(d.decode()?),
                Some(RequestMessageCborKey::Endpoint) => builder.method(d.decode()?),
                Some(RequestMessageCborKey::Argument) => builder.data(d.bytes()?.to_vec()),
                Some(RequestMessageCborKey::Timestamp) => {
                    // Some logic applies.
                    let t = d.tag()?;
                    if t != minicbor::data::Tag::Timestamp {
                        return Err(minicbor::decode::Error::Message("Invalid tag."));
                    } else {
                        let secs = d.u64()?;
                        let timestamp = std::time::UNIX_EPOCH
                            .checked_add(Duration::from_secs(secs))
                            .ok_or(minicbor::decode::Error::Message(
                                "duration value can not represent system time",
                            ))?;
                        builder.timestamp(timestamp)
                    }
                }
                Some(RequestMessageCborKey::Id) => builder.id(d.u64()?),
                Some(RequestMessageCborKey::Nonce) => builder.nonce(d.bytes()?.to_vec()),
            };

            i += 1;
            if x.map_or(false, |x| i >= x) {
                break;
            }
        }

        builder
            .build()
            .map_err(|_e| minicbor::decode::Error::Message("could not build"))
    }
}
