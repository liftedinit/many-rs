use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use num_derive::{FromPrimitive, ToPrimitive};
use std::collections::BTreeMap;

#[derive(FromPrimitive, ToPrimitive)]
#[repr(i8)]
enum ReasonCborKey {
    Code = 0,
    Message = 1,
    Arguments = 2,
}

impl<T: Encode<()>> crate::Reason<T> {
    #[inline]
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        minicbor::encode(self, &mut bytes).map_err(|e| format!("{e}"))?;
        Ok(bytes)
    }
}

impl<'b, T: Decode<'b, ()> + Default> crate::Reason<T> {
    #[inline]
    pub fn from_bytes(bytes: &'b [u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{e}"))
    }
}

impl<T: Encode<C>, C> Encode<C> for crate::Reason<T> {
    #[inline]
    fn encode<W: Write>(&self, e: &mut Encoder<W>, ctx: &mut C) -> Result<(), Error<W::Error>> {
        e.map(1 + u64::from(self.message.is_some()) + u64::from(!self.arguments.is_empty()))?
            .u32(ReasonCborKey::Code as u32)?
            .encode_with(&self.code, ctx)?;

        if let Some(msg) = &self.message {
            e.u32(ReasonCborKey::Message as u32)?.str(msg.as_str())?;
        }
        if !self.arguments.is_empty() {
            e.u32(ReasonCborKey::Arguments as u32)?
                .encode(&self.arguments)?;
        }
        Ok(())
    }
}

impl<'b, T: Decode<'b, C> + Default, C> Decode<'b, C> for crate::Reason<T> {
    fn decode(d: &mut Decoder<'b>, ctx: &mut C) -> Result<Self, minicbor::decode::Error> {
        let len = d.map()?;

        let mut code: Option<T> = None;
        let mut message = None;
        let mut arguments: BTreeMap<String, String> = BTreeMap::new();

        let mut i = 0;
        loop {
            if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            match num_traits::FromPrimitive::from_i64(d.i64()?) {
                Some(ReasonCborKey::Code) => code = Some(d.decode_with(ctx)?),
                Some(ReasonCborKey::Message) => message = Some(d.str()?),
                Some(ReasonCborKey::Arguments) => arguments = d.decode()?,
                None => {}
            }

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        Ok(Self {
            code: code.unwrap_or_default(),
            message: message.map(|s| s.to_string()),
            arguments,
        })
    }
}
