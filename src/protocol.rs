use crate::Identity;
use derive_builder::Builder;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use minicose::CoseKey;
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};

pub mod attributes;

#[derive(Clone)]
pub enum AttributeArgument {
    Bool(bool),
    Int(i64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<AttributeArgument>),
}

impl Debug for AttributeArgument {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AttributeArgument::Bool(b) => write!(f, "{}", b),
            AttributeArgument::Int(i) => write!(f, "{}", i),
            AttributeArgument::String(s) => f.write_str(s),
            AttributeArgument::Bytes(b) => write!(f, r#"b"{}""#, hex::encode(b)),
            AttributeArgument::Array(a) => write!(f, "{:?}", a),
        }
    }
}

impl Encode for AttributeArgument {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        match self {
            AttributeArgument::Bool(b) => {
                e.bool(*b)?;
            }
            AttributeArgument::Int(i) => {
                e.i64(*i)?;
            }
            AttributeArgument::String(s) => {
                e.str(s)?;
            }
            AttributeArgument::Bytes(b) => {
                e.bytes(b)?;
            }
            AttributeArgument::Array(arr) => {
                e.array(arr.len() as u64)?;
                for ref i in arr {
                    e.encode(i)?;
                }
            }
        }

        Ok(())
    }
}

impl<'d> Decode<'d> for AttributeArgument {
    fn decode(d: &mut Decoder<'d>) -> Result<Self, minicbor::decode::Error> {
        match d.datatype()? {
            Type::Bool => Ok(AttributeArgument::Bool(d.bool()?)),
            Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
            | Type::I8
            | Type::I16
            | Type::I32
            | Type::I64 => Ok(AttributeArgument::Int(d.i64()?)),
            Type::Bytes => Ok(AttributeArgument::Bytes(d.bytes()?.to_vec())),
            Type::String => Ok(AttributeArgument::String(d.str()?.to_string())),
            Type::ArrayIndef | Type::Array => {
                Ok(AttributeArgument::Array(d.array_iter()?.collect::<Result<
                    Vec<AttributeArgument>,
                    minicbor::decode::Error,
                >>(
                )?))
            }
            _ => Err(minicbor::decode::Error::Message(
                "Invalid data type while decoding arguments.",
            )),
        }
    }
}

#[derive(Clone)]
pub struct Attribute {
    pub id: u32,
    pub arguments: Vec<AttributeArgument>,

    /// If this is `None`, that means this attribute came from the wire and these should not be
    /// relied upon. Endpoints are just meta information given when building modules and they
    /// are not encoded/decoded in status.
    pub endpoints: Option<&'static [&'static str]>,
}

impl Attribute {
    const fn just_id(id: u32) -> Self {
        Self {
            id,
            arguments: Vec::new(),
            endpoints: None,
        }
    }

    pub const fn new(id: u32, endpoints: &'static [&'static str]) -> Self {
        Self {
            id,
            arguments: Vec::new(),
            endpoints: Some(endpoints),
        }
    }

    pub fn with_arguments(self, arguments: Vec<AttributeArgument>) -> Self {
        Self { arguments, ..self }
    }

    pub fn with_argument(mut self, argument: AttributeArgument) -> Self {
        self.arguments.push(argument);
        self
    }
}

impl PartialEq for Attribute {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Attribute {}

impl PartialOrd for Attribute {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl Ord for Attribute {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl Debug for Attribute {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("Attribute");

        dbg.field("id", &self.id)
            .field("arguments", &self.arguments);

        if let Some(ep) = self.endpoints {
            dbg.field("endpoints", &ep.to_vec());
        }
        dbg.finish()
    }
}

impl Encode for Attribute {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        if !self.arguments.is_empty() {
            e.array(1 + self.arguments.len() as u64)?;
        }

        e.u32(self.id as u32)?;

        if !self.arguments.is_empty() {
            for a in &self.arguments {
                e.encode(a)?;
            }
        }

        Ok(())
    }
}

impl<'d> Decode<'d> for Attribute {
    fn decode(d: &mut Decoder<'d>) -> Result<Self, minicbor::decode::Error> {
        match d.datatype()? {
            Type::Array | Type::ArrayIndef => {
                let len = d.array()?;
                let id = d.u32()?;
                let mut arguments =
                    Vec::<AttributeArgument>::with_capacity(len.unwrap_or(8) as usize);

                let mut i = 0;
                loop {
                    if d.datatype()? == Type::Break {
                        d.skip()?;
                        break;
                    }

                    arguments.push(d.decode()?);

                    i += 1;
                    if len.map_or(false, |x| i >= x) {
                        break;
                    }
                }

                Ok(Self {
                    id,
                    arguments,
                    endpoints: None,
                })
            }
            _ => Ok(Self::just_id(d.u32()? as u32)),
        }
    }
}

#[derive(Clone, Debug, Builder)]
pub struct Status {
    pub name: String,
    pub version: u8,
    pub public_key: Option<CoseKey>,
    pub internal_version: String,
    pub identity: Identity,
    pub attributes: Vec<Attribute>,
}

impl Status {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        minicbor::to_vec(self).map_err(|e| format!("{}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }
}

impl Encode for Status {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        #[rustfmt::skip]
        e.begin_map()?
            .str("name")?.str(self.name.as_str())?
            .str("version")?.u8(self.version)?
            .str("identity")?.encode(&self.identity)?
            .str("internal_version")?.str(self.internal_version.as_str())?
            .str("attributes")?.encode(self.attributes.as_slice())?;

        if let Some(pk) = self.public_key.as_ref() {
            let public_key = pk.to_public_key().unwrap().to_bytes().unwrap();
            e.str("public_key")?.bytes(public_key.as_slice())?;
        }

        e.end()?;

        Ok(())
    }
}

impl<'b> Decode<'b> for Status {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let mut builder = StatusBuilder::default();
        let len = d.map()?;
        let mut i = 0;

        loop {
            if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            match d.str()? {
                "name" => builder.name(d.str()?.to_string()),
                "version" => builder.version(d.decode()?),
                "public_key" => {
                    let bytes = d.bytes()?;
                    let key: CoseKey = CoseKey::from_bytes(bytes)
                        .map_err(|_e| minicbor::decode::Error::Message("Invalid cose key."))?;
                    builder.public_key(Some(key))
                }
                "internal_version" => builder.internal_version(d.str()?.to_string()),
                "identity" => builder.identity(d.decode()?),
                "attributes" => builder.attributes(d.decode()?),
                _ => &mut builder,
            };

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        builder
            .build()
            .map_err(|_e| minicbor::decode::Error::Message("could not build"))
    }
}
