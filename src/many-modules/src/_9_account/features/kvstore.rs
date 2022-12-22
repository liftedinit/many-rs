use crate::account::features::{Feature, FeatureId, TryCreateFeature};
use crate::account::Role;
use many_error::ManyError;
use std::collections::BTreeSet;

pub struct AccountKvStore;

impl TryCreateFeature for AccountKvStore {
    const ID: FeatureId = 2;

    fn try_create(_: &Feature) -> Result<Self, ManyError> {
        Ok(Self)
    }
}

impl super::FeatureInfo for AccountKvStore {
    fn as_feature(&self) -> Feature {
        Feature::with_id(Self::ID)
    }

    fn roles() -> BTreeSet<Role> {
        BTreeSet::from([
            Role::CanKvStorePut,
            Role::CanKvStoreDisable,
            Role::CanKvStoreTransfer,
        ])
    }
}
