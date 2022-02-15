use crate::cbor::CborAny;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};

pub mod attributes;

pub type AttributeId = u32;

#[derive(Clone)]
pub struct Attribute {
    pub id: AttributeId,
    pub arguments: Option<Vec<CborAny>>,
}

impl Attribute {
    pub const fn id(id: AttributeId) -> Self {
        Self {
            id,
            arguments: None,
        }
    }

    pub const fn new(id: AttributeId, arguments: Vec<CborAny>) -> Self {
        Self {
            id,
            arguments: Some(arguments),
        }
    }

    pub fn with_arguments(&self, arguments: Vec<CborAny>) -> Self {
        Self {
            arguments: Some(arguments),
            ..self.clone()
        }
    }

    pub fn with_argument(&self, argument: CborAny) -> Self {
        let mut arguments = self.arguments.as_ref().cloned().unwrap_or_default();
        arguments.push(argument);
        Self {
            arguments: Some(arguments),
            id: self.id,
        }
    }

    pub fn into_arguments(self) -> Vec<CborAny> {
        if let Some(a) = self.arguments {
            a
        } else {
            vec![]
        }
    }

    pub fn arguments(&self) -> Option<&Vec<CborAny>> {
        match &self.arguments {
            Some(arguments) => {
                if arguments.is_empty() {
                    None
                } else {
                    Some(arguments)
                }
            }
            None => None,
        }
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
        if let Some(ref arguments) = self.arguments() {
            f.debug_struct("Attribute")
                .field("id", &self.id)
                .field("arguments", &arguments)
                .finish()
        } else {
            f.write_fmt(format_args!("Attribute({})", self.id))
        }
    }
}

impl Encode for Attribute {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        if let Some(arguments) = self.arguments() {
            e.array(1 + arguments.len() as u64)?;
            e.u32(self.id as u32)?;
            for a in arguments {
                e.encode(a)?;
            }
        } else {
            e.u32(self.id as u32)?;
        }

        Ok(())
    }
}

impl<'d> Decode<'d> for Attribute {
    fn decode(d: &mut Decoder<'d>) -> Result<Self, minicbor::decode::Error> {
        match d.datatype()? {
            Type::Array | Type::ArrayIndef => {
                let arr = d.array_iter()?.collect::<Result<Vec<CborAny>, _>>()?;
                let (id, arguments) = arr
                    .as_slice()
                    .split_first()
                    .ok_or_else(|| minicbor::decode::Error::Message("Invalid empty attribute."))?;

                match id {
                    CborAny::Int(i) if i <= &(u32::MAX as i64) => Ok(Self {
                        id: *i as u32,
                        arguments: Some(arguments.to_vec()),
                    }),
                    _ => Err(minicbor::decode::Error::Message(
                        "Expected an attribute ID.",
                    )),
                }
            }
            _ => Ok(Self::id(d.u32()? as u32)),
        }
    }
}
