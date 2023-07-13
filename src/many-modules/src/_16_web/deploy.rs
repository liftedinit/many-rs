use minicbor::{Decode, Encode};
use many_types::web::WebDeploymentSource;

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct DeployArgs {
    #[n(0)]
    pub site_name: String,

    #[n(1)]
    pub site_description: Option<String>,

    #[n(2)]
    pub source: WebDeploymentSource,
}

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct DeployReturns {
    #[n(0)]
    pub url: String,
}
