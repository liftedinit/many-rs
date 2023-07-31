use many_identity::Address;
use many_types::compute::{ComputeListFilter, DeploymentMeta};
use many_types::SortOrder;
use minicbor::{Decode, Encode};

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct ListArgs {
    #[n(0)]
    pub owner: Option<Address>,

    #[n(1)]
    pub order: Option<SortOrder>,

    #[n(2)]
    pub filter: Option<ComputeListFilter>,
}

#[derive(Clone, Decode, Encode)]
#[cbor(map)]
pub struct ListReturns {
    #[n(0)]
    pub deployments: Vec<DeploymentMeta>,
}
