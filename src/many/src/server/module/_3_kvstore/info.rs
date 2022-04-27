use minicbor::bytes::ByteVec;
use minicbor::encode::{Error, Write};
use minicbor::{decode, Decode, Decoder, Encode, Encoder};

pub struct InfoArgs;
impl<'de> Decode<'de> for InfoArgs {
    fn decode(_d: &mut Decoder<'de>) -> Result<Self, decode::Error> {
        Ok(Self)
    }
}

impl Encode for InfoArgs {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.null()?.ok()
    }
}

#[derive(Decode, Encode)]
pub struct InfoReturns {
    #[n(0)]
    pub hash: ByteVec,
}
