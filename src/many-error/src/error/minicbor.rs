use crate::{ManyError, ManyErrorCode};
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};

impl<C> Encode<C> for ManyErrorCode {
    #[inline]
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        e.i64((*self).into())?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for ManyErrorCode {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        Ok(d.i64()?.into())
    }
}

impl<C> Encode<C> for ManyError {
    #[inline]
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        e.encode(&self.0)?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for ManyError {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        Ok(Self(d.decode()?))
    }
}
