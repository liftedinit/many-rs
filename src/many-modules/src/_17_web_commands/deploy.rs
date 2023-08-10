use many_identity::Address;
use many_types::web::{WebDeploymentInfo, WebDeploymentSource};
use many_types::Memo;
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

    #[n(4)]
    pub memo: Option<Memo>,
}

#[derive(Clone, Debug, Decode, Encode, PartialEq, Eq)]
#[cbor(map)]
pub struct DeployReturns {
    #[n(0)]
    pub info: WebDeploymentInfo,
}
