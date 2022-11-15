use minicbor::bytes::ByteVec;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};

const MULTISIG_MEMO_DATA_MAX_SIZE: usize = 4000; //4kB

/// A short note in a transaction
#[derive(Clone, Debug, Eq, PartialEq)]
// Using AsRef<str> for extensibility:
// We might want to borrow with a Memo<&str> instead of a &Memo<String>
pub struct Memo<S: AsRef<str>>(S);

impl<S> AsRef<str> for Memo<S>
where
    S: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<S> ToString for Memo<S>
where
    S: AsRef<str>,
{
    fn to_string(&self) -> String {
        self.0.as_ref().to_string()
    }
}

impl<S, C> Encode<C> for Memo<S>
where
    S: AsRef<str>,
{
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.str(self.0.as_ref()).map(|_| ())
    }
}

impl<C> Decode<'_, C> for Memo<String> {
    fn decode(d: &mut Decoder<'_>, ctx: &mut C) -> Result<Self, decode::Error> {
        let Memo(s): Memo<&str> = Memo::decode(d, ctx)?;
        Ok(Memo(s.into()))
    }
}

// Implementing for completeness, in case we want zero-allocation parsing in the future
impl<'b, C> Decode<'b, C> for Memo<&'b str> {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        let s = d.str()?;
        if s.as_bytes().len() > MULTISIG_MEMO_DATA_MAX_SIZE {
            return Err(decode::Error::message("Memo size over limit"));
        }
        Ok(Memo(s))
    }
}

impl TryFrom<String> for Memo<String> {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.as_str().as_bytes().len() > MULTISIG_MEMO_DATA_MAX_SIZE {
            return Err(format!(
                "Memo size over limit {}",
                s.as_str().as_bytes().len()
            ));
        }
        Ok(Memo(s))
    }
}

impl Memo<String> {
    // USED ONLY IN TESTS! NEVER REMOVE THIS GUARD! MEMOS SHOULDN'T BE PRODUCED FROM LARGE STRINGS
    #[cfg(test)]
    pub fn new(s: String) -> Self {
        Memo(s)
    }
}

/// Data inside a transaction
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Data(pub(crate) ByteVec);

impl<C> Encode<C> for Data {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.bytes(self.0.as_ref()).map(|_| ())
    }
}

impl<C> Decode<'_, C> for Data {
    fn decode(d: &mut Decoder<'_>, _: &mut C) -> Result<Self, decode::Error> {
        let b = d.bytes()?;
        if b.len() > MULTISIG_MEMO_DATA_MAX_SIZE {
            return Err(decode::Error::message("Data size over limit"));
        }
        Ok(Data(b.to_vec().into()))
    }
}

impl TryFrom<Vec<u8>> for Data {
    type Error = String;

    fn try_from(b: Vec<u8>) -> Result<Self, Self::Error> {
        if b.len() > MULTISIG_MEMO_DATA_MAX_SIZE {
            return Err(format!("Data size over limit {}", b.len()));
        }
        Ok(Data(b.into()))
    }
}

impl Data {
    // USED ONLY IN TESTS! NEVER REMOVE THIS GUARD! MEMOS SHOULDN'T BE PRODUCED FROM LARGE STRINGS
    #[cfg(test)]
    #[allow(unused)]
    fn new(b: ByteVec) -> Self {
        Data(b)
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}
