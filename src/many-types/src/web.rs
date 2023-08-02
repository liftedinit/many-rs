use std::str::FromStr;
use many_identity::Address;
use minicbor::{Decode, Encode};
use minicbor::bytes::ByteVec;
use strum::Display;
use many_error::ManyError;

#[derive(Clone, Debug, Decode, Display, Encode, Eq, PartialEq)]
#[cbor(map)]
pub enum WebDeploymentFilter {
    #[n(0)]
    All,

    #[n(1)]
    Owner(#[n(0)] Address),
}

impl FromStr for WebDeploymentFilter {
    type Err = ManyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(WebDeploymentFilter::All),
            s if s.starts_with("owner:") => {
                let address = s.trim_start_matches("owner:");
                let address = Address::from_str(address)?;
                Ok(WebDeploymentFilter::Owner(address))
            }
            _ => Err(ManyError::unknown("invalid filter")),
        }
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
    pub url: Option<String>,
}

#[derive(Clone, Debug, Encode, Decode, Display, Eq, PartialEq)]
#[cbor(map)]
pub enum WebDeploymentSource {
    #[n(0)]
    Zip(#[n(0)] ByteVec)
}
