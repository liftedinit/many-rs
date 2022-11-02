use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};

pub enum SingleVisualTokenLogo {
    /// A single character. This is limited to a single character for now.
    UnicodeChar(char),
    Image {
        content_type: String,
        binary: Vec<u8>,
    },
}

impl SingleVisualTokenLogo {
    pub fn char(c: char) -> Self {
        Self::UnicodeChar(c)
    }
    pub fn image(content_type: String, binary: Vec<u8>) -> Self {
        Self::Image {
            content_type,
            binary,
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
                e.map(2)?.u8(0)?.u8(0)?.u8(1)?.str(&String::from(*c))?;
            }
            SingleVisualTokenLogo::Image {
                content_type,
                binary,
            } => {
                e.map(3)?
                    .u8(0)?
                    .u8(0)?
                    .u8(1)?
                    .str(content_type)?
                    .u8(2)?
                    .bytes(&binary)?;
            }
        }
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for SingleVisualTokenLogo {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        let mut l = d.map()?.ok_or(decode::Error::message(
            "Indefinite length map not supported",
        ))?;
        if l < 2 {
            return Err(decode::Error::end_of_input());
        }
        if d.u8()? != 0 {
            return Err(decode::Error::message("Expected key 0 first"));
        }

        let this = match d.u8()? {
            0 => {
                // Unicode character.
                if d.u8()? != 1 {
                    Err(decode::Error::message("Expected key 1"))
                } else {
                    l -= 1;
                    Ok(Self::char(d.str()?.chars().next().ok_or(
                        decode::Error::message("Unicode character empty"),
                    )?))
                }
            }
            1 => {
                // Visual Token.
                let mut content_type = None;
                let mut binary = None;
                for _ in 1..l {
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
                    content_type.ok_or(decode::Error::message("Missing content type."))?,
                    binary.ok_or(decode::Error::message("Missing binary data."))?,
                ))
            }
            i => Err(decode::Error::message(format!("Unknown key {i}"))),
        }?;
        if l > 0 {
            return Err(decode::Error::message("Too many keys in the map."));
        }

        Ok(this)
    }
}

#[derive(Default)]
pub struct VisualTokenLogo(Vec<SingleVisualTokenLogo>);

impl VisualTokenLogo {
    pub fn unicode(&mut self, c: char) {
        self.0.push(SingleVisualTokenLogo::char(c))
    }
}
