use many_identity::Address;
use many_types::web::WebDeploymentSource;
use minicbor::{Decode, Encode};

#[derive(Clone, Debug, Decode, Encode, PartialEq, Eq)]
#[cbor(map)]
pub struct DeployArgs {
    #[n(0)]
    pub owner: Option<Address>,

    #[n(1)]
    pub site_name: String,

    #[n(2)]
    pub site_description: Option<String>,

    #[n(3)]
    pub source: WebDeploymentSource,
}

#[derive(Clone, Debug, Decode, Encode, PartialEq, Eq)]
#[cbor(map)]
pub struct DeployReturns {
    #[n(0)]
    pub url: String,
}
