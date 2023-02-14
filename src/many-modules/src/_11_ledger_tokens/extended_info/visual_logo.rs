use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use num_enum::TryFromPrimitive;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SingleVisualTokenLogo {
    /// A single character. This is limited to a single character for now.
    UnicodeChar(char), // TODO: Match spec. Do not limit to a single char
    Image {
        content_type: String,
        binary: Arc<Vec<u8>>,
    },
}

#[derive(Copy, Clone, Debug, Decode, Encode, Ord, PartialOrd, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
#[cbor(index_only)]
pub enum SingleVisualTokenLogoKey {
    #[n(0)]
    UnicodeChar = 0,

    #[n(1)]
    Image = 1,
}

impl SingleVisualTokenLogo {
    pub fn as_key(&self) -> SingleVisualTokenLogoKey {
        match self {
            SingleVisualTokenLogo::UnicodeChar(_) => SingleVisualTokenLogoKey::UnicodeChar,
            SingleVisualTokenLogo::Image { .. } => SingleVisualTokenLogoKey::Image,
        }
    }
    pub fn char(c: char) -> Self {
        Self::UnicodeChar(c)
    }
    pub fn image(content_type: impl AsRef<str>, binary: Vec<u8>) -> Self {
        Self::Image {
            content_type: content_type.as_ref().into(),
            binary: Arc::new(binary),
        }
    }
}

impl<C> Encode<C> for SingleVisualTokenLogo {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _ctx: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        match self {
            SingleVisualTokenLogo::UnicodeChar(c) => {
                e.map(2)?
                    .u8(0)?
                    .u8(SingleVisualTokenLogoKey::UnicodeChar as u8)?
                    .u8(1)?
                    .str(&String::from(*c))?;
            }
            SingleVisualTokenLogo::Image {
                content_type,
                binary,
            } => {
                e.map(3)?
                    .u8(0)?
                    .u8(SingleVisualTokenLogoKey::Image as u8)?
                    .u8(1)?
                    .str(content_type)?
                    .u8(2)?
                    .bytes(binary)?;
            }
        }
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for SingleVisualTokenLogo {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        let mut l = d
            .map()?
            .ok_or_else(|| decode::Error::message("Indefinite length map not supported"))?;
        if l < 2 {
            return Err(decode::Error::end_of_input());
        }
        if d.u8()? != 0 {
            return Err(decode::Error::message("Expected key 0 first"));
        }

        let key: SingleVisualTokenLogoKey = d.decode()?;
        let this = match key {
            SingleVisualTokenLogoKey::UnicodeChar => {
                if d.u8()? != 1 {
                    Err(decode::Error::message("Expected key 1"))
                } else {
                    l -= 1;
                    Ok(Self::char(d.str()?.chars().next().ok_or_else(|| {
                        decode::Error::message("Unicode character empty")
                    })?))
                }
            }
            SingleVisualTokenLogoKey::Image => {
                let mut content_type = None;
                let mut binary = None;
                let l_ = l; // Silence warning
                for _ in 1..l_ {
                    match d.u8()? {
                        1 => {
                            content_type = Some(d.str()?.to_string());
                        }
                        2 => {
                            binary = Some(d.bytes()?.to_vec());
                        }
                        i => {
                            return Err(decode::Error::message(format!("Unknown key {i}")));
                        }
                    }
                    l -= 1;
                }
                Ok(Self::image(
                    content_type.ok_or_else(|| decode::Error::message("Missing content type."))?,
                    binary.ok_or_else(|| decode::Error::message("Missing binary data."))?,
                ))
            }
        }?;
        if l != 1 {
            return Err(decode::Error::message("Too many keys in the map."));
        }

        Ok(this)
    }
}

#[derive(Default, Clone, Debug, Encode, Decode, PartialEq, Eq)]
#[cbor(transparent)]
pub struct VisualTokenLogo(#[n(0)] VecDeque<SingleVisualTokenLogo>);

impl VisualTokenLogo {
    pub fn new() -> Self {
        Self(Default::default())
    }
    pub fn unicode_front(&mut self, c: char) {
        self.0.push_front(SingleVisualTokenLogo::char(c))
    }
    pub fn image_front(&mut self, content_type: impl AsRef<str>, data: Vec<u8>) {
        self.0
            .push_front(SingleVisualTokenLogo::image(content_type, data))
    }
    pub fn unicode_back(&mut self, c: char) {
        self.0.push_back(SingleVisualTokenLogo::char(c))
    }
    pub fn image_back(&mut self, content_type: impl AsRef<str>, data: Vec<u8>) {
        self.0
            .push_back(SingleVisualTokenLogo::image(content_type, data))
    }

    pub fn sort(
        &mut self,
        sorting_fn: impl Fn(&SingleVisualTokenLogo, &SingleVisualTokenLogo) -> Ordering,
    ) {
        self.0.make_contiguous().sort_by(sorting_fn);
    }
}

impl Deref for VisualTokenLogo {
    type Target = VecDeque<SingleVisualTokenLogo>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_unicode_char() {
        let logo = SingleVisualTokenLogo::char('∑');

        let enc = minicbor::to_vec(logo).unwrap();
        let res: SingleVisualTokenLogo = minicbor::decode(&enc).unwrap();

        match res {
            SingleVisualTokenLogo::UnicodeChar(c) => assert_eq!(c, '∑'),
            _ => panic!("Invalid logo type"),
        }
    }

    #[test]
    fn encode_decode_image() {
        let logo = SingleVisualTokenLogo::image("png", vec![1u8; 10]);

        let enc = minicbor::to_vec(logo).unwrap();
        let res: SingleVisualTokenLogo = minicbor::decode(&enc).unwrap();

        match res {
            SingleVisualTokenLogo::Image {
                content_type,
                binary,
            } => {
                assert_eq!(content_type, "png");
                assert_eq!(*binary, vec![1u8; 10]);
            }
            _ => panic!("Invalid logo type"),
        }
    }

    #[test]
    fn encode_decode_logos() {
        let mut logos = VisualTokenLogo::new();
        logos.unicode_front('∑');
        logos.unicode_back('π');
        logos.image_front("foo", vec![2u8; 10]);
        logos.image_back("bar", vec![5u8; 20]);

        let enc = minicbor::to_vec(&logos).unwrap();
        let res: VisualTokenLogo = minicbor::decode(&enc).unwrap();

        for (i, j) in logos.iter().zip(res.iter()) {
            assert_eq!(i, j);
        }
    }

    #[test]
    fn sort() {
        let mut logos = VisualTokenLogo::new();
        logos.unicode_front('∑');
        logos.image_front("foo", vec![2u8; 10]);
        logos.image_back("bar", vec![5u8; 20]);
        logos.unicode_back('π');

        let mut iter = logos.iter();
        assert!(matches!(
            iter.next().unwrap(),
            SingleVisualTokenLogo::Image { .. }
        ));
        assert!(matches!(
            iter.next().unwrap(),
            SingleVisualTokenLogo::UnicodeChar(_)
        ));
        assert!(matches!(
            iter.next().unwrap(),
            SingleVisualTokenLogo::Image { .. }
        ));
        assert!(matches!(
            iter.next().unwrap(),
            SingleVisualTokenLogo::UnicodeChar(_)
        ));

        logos.sort(|a, b| a.as_key().cmp(&b.as_key()));

        let mut iter = logos.iter();
        assert!(matches!(
            iter.next().unwrap(),
            SingleVisualTokenLogo::UnicodeChar(_)
        ));
        assert!(matches!(
            iter.next().unwrap(),
            SingleVisualTokenLogo::UnicodeChar(_)
        ));
        assert!(matches!(
            iter.next().unwrap(),
            SingleVisualTokenLogo::Image { .. }
        ));
        assert!(matches!(
            iter.next().unwrap(),
            SingleVisualTokenLogo::Image { .. }
        ));
    }
}
