use minicbor::encode::{Error, Write};
use minicbor::{Decode, Encode, Encoder};
use strum::Display;
use many_identity::Address;

#[derive(Clone, Debug, Display, Eq, PartialEq)]
pub enum WebDeploymentFilter {
    All,
    Owner(Address),
}

impl<C> minicbor::Encode<C> for WebDeploymentFilter {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        match self {
            WebDeploymentFilter::All => {
                e.map(1)?.u8(0)?;
            }
            WebDeploymentFilter::Owner(addr) => {
                e.map(1)?.u8(1)?.str(&addr.to_string())?;
            }
        }
        Ok(())
    }
}

impl<'d, C> minicbor::Decode<'d, C> for WebDeploymentFilter {
    fn decode(d: &mut minicbor::Decoder<'d>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        let mut indefinite = false;
        let key = match d.map()? {
            None => {
                indefinite = true;
                d.u8()
            }
            Some(1) => d.u8(),
            Some(_) => Err(minicbor::decode::Error::message(
                "Invalid length for web deployment filter map.",
            )),
        }?;
        let result = match key {
            0 => Ok(WebDeploymentFilter::All),
            1 => Ok(WebDeploymentFilter::Owner(d.str()?.to_string().parse().map_err(|_| {
                    minicbor::decode::Error::message("invalid address".to_string())
                })?)),
            x => Err(minicbor::decode::Error::unknown_variant(u32::from(x))),
        };

        if indefinite {
            d.skip()?;
        }

        result
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Encode, Decode)]
#[cbor(map)]
pub struct WebDeploymentInfo {
    #[n(0)]
    pub site_name: String,

    #[n(1)]
    pub site_description: Option<String>,

    #[n(2)]
    pub source: WebDeploymentSource,

    #[n(3)]
    pub url: String,
}

#[derive(Clone, Debug, Display, Eq, PartialEq)]
pub enum WebDeploymentSource {
    GitHub(String)
}

impl<C> minicbor::Encode<C> for WebDeploymentSource {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        match self {
            WebDeploymentSource::GitHub(s) => {
                e.map(1)?.u8(0)?.str(s)?;
            }
        }
        Ok(())
    }
}

impl<'d, C> minicbor::Decode<'d, C> for WebDeploymentSource {
    fn decode(d: &mut minicbor::Decoder<'d>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
        let mut indefinite = false;
        let key = match d.map()? {
            None => {
                indefinite = true;
                d.u8()
            }
            Some(1) => d.u8(),
            Some(_) => Err(minicbor::decode::Error::message(
                "Invalid length for web deployment source map.",
            )),
        }?;

        let result = match key {
            0 => Ok(WebDeploymentSource::GitHub(d.str()?.to_string())),
            x => Err(minicbor::decode::Error::unknown_variant(u32::from(x))),
        };

        if indefinite {
            d.skip()?;
        }

        result
    }
}