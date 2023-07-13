use minicbor::{Decode, Encode};
use many_types::SortOrder;
use many_types::web::{WebDeploymentFilter, WebDeploymentInfo};

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct ListArgs {
    #[n(0)]
    pub order: Option<SortOrder>,

    #[n(1)]
    pub filter: Option<WebDeploymentFilter>,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct ListReturns {
    #[n(0)]
    pub deployments: Vec<WebDeploymentInfo>,
}