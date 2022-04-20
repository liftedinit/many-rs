use crate::cbor::CborAny;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};

pub mod attributes;
pub use attributes::AttributeSet;

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
                    .ok_or(minicbor::decode::Error::Message("Invalid empty attribute."))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    proptest::proptest! {
        #[test]
        fn attribute(seedu32: u32, seedi64: i64) {
            let att = Attribute::id(seedu32);
            assert_eq!(att.id, seedu32);
            assert_eq!(att.arguments, None);
            assert_eq!(att.arguments(), None);


            let arguments = vec![CborAny::Bytes(vec![0; 8])];
            let att = Attribute::new(seedu32, arguments.clone());
            assert_eq!(att.id, seedu32);
            assert_eq!(att.arguments, Some(arguments));

            let arguments = vec![CborAny::Int(seedi64)];
            let att = att.with_arguments(arguments.clone());
            assert_eq!(att.id, seedu32);
            assert_eq!(att.arguments, Some(arguments));

            let argument = CborAny::String("Foobar".to_string());
            let att = Attribute::id(seedu32);
            let att = att.with_argument(argument.clone());
            assert_eq!(att.id, seedu32);
            assert_eq!(att.arguments, Some(vec![argument.clone()]));
            let att = att.with_argument(argument.clone());
            assert_eq!(att.arguments, Some(vec![argument.clone(), argument.clone()]));

            let arguments = att.clone().into_arguments();
            assert_eq!(arguments, vec![argument.clone(), argument.clone()]);
            assert_eq!(att.arguments(), Some(&vec![argument.clone(), argument]));

            assert_eq!(att, att);

            let att = Attribute::id(0);
            let att2 = Attribute::id(1);
            assert_eq!(att.cmp(&att2), Ordering::Less);
            assert_eq!(att.partial_cmp(&att2), Some(Ordering::Less));

            let att = Attribute::new(seedu32, vec![CborAny::Int(seedi64)]);
            let att_cbor_enc = minicbor::to_vec(&att).unwrap();
            let att_cbor_dec: Attribute = minicbor::decode(&att_cbor_enc).unwrap();
            assert_eq!(att_cbor_dec, att);
        }
    }
}
