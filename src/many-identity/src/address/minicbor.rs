use minicbor::data::Type;
use minicbor::encode::Write;
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::str::FromStr;

impl<C> Encode<C> for crate::Address {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.tag(minicbor::data::Tag::Unassigned(10000))?
            .bytes(&self.to_vec())?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for crate::Address {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        let mut is_tagged = false;
        // Check all the tags.
        while d.datatype()? == Type::Tag {
            if d.tag()? == minicbor::data::Tag::Unassigned(10000) {
                is_tagged = true;
            }
        }

        match d.datatype()? {
            Type::String => Self::from_str(d.str()?),
            _ => {
                if !is_tagged {
                    return Err(minicbor::decode::Error::message(
                        "identities need to be tagged",
                    ));
                }

                Self::try_from(d.bytes()?)
            }
        }
        .map_err(|_e| minicbor::decode::Error::message("Could not decode identity from bytes"))
    }
}
