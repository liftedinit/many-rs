use crate::cbor::CborAny;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};

pub mod attributes;

#[derive(Clone)]
pub struct Attribute {
    pub id: u32,
    pub arguments: Vec<CborAny>,
}

impl Attribute {
    pub const fn id(id: u32) -> Self {
        Self {
            id,
            arguments: Vec::new(),
        }
    }

    pub const fn new(id: u32, arguments: Vec<CborAny>) -> Self {
        Self { id, arguments }
    }

    pub fn with_arguments(&self, arguments: Vec<CborAny>) -> Self {
        Self {
            arguments,
            ..self.clone()
        }
    }

    pub fn with_argument(&self, argument: CborAny) -> Self {
        let mut s = self.clone();
        s.arguments.push(argument);
        s
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
        if self.arguments.is_empty() {
            f.write_fmt(format_args!("Attribute({})", self.id))
        } else {
            f.debug_struct("Attribute")
                .field("id", &self.id)
                .field("arguments", &self.arguments)
                .finish()
        }
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
                let mut arguments = Vec::<CborAny>::with_capacity(len.unwrap_or(8) as usize);

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

                Ok(Self { id, arguments })
            }
            _ => Ok(Self::id(d.u32()? as u32)),
        }
    }
}
