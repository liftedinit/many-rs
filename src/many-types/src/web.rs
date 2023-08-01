use many_identity::Address;
use minicbor::{Decode, Encode};
use strum::Display;

#[derive(Clone, Debug, Decode, Display, Encode, Eq, PartialEq)]
#[cbor(map)]
pub enum WebDeploymentFilter {
    #[n(0)]
    All,

    #[n(1)]
    Owner(#[n(0)] Address),
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
    GitHub(#[n(0)] String, #[n(1)] Option<String>), // Github("repo url", "build artifact path")
}
