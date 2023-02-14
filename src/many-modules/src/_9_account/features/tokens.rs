/// See feature `_0_account_ledger`.
use crate::account::features::{Feature, FeatureId, TryCreateFeature};
use crate::account::Role;
use many_error::ManyError;
use std::collections::BTreeSet;

pub struct TokenAccountLedger;

impl TryCreateFeature for TokenAccountLedger {
    const ID: FeatureId = 3;

    fn try_create(_: &Feature) -> Result<Self, ManyError> {
        Ok(Self)
    }
}

impl super::FeatureInfo for TokenAccountLedger {
    fn as_feature(&self) -> Feature {
        Feature::with_id(Self::ID)
    }

    fn roles() -> BTreeSet<Role> {
        BTreeSet::from([
            Role::CanTokensCreate,
            Role::CanTokensBurn,
            Role::CanTokensMint,
            Role::CanTokensUpdate,
            Role::CanTokensAddExtendedInfo,
            Role::CanTokensRemoveExtendedInfo,
        ])
    }
}
