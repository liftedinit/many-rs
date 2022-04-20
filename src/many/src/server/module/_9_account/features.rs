/// See feature `_0_account_ledger`.
pub mod ledger {
    use crate::server::module::_9_account::FeatureSet;
    use crate::server::module::account::{Feature, TryFromFeatureSet};
    use crate::ManyError;

    pub const ACCOUNT_LEDGER: Feature = Feature::with_id(0);

    pub struct AccountLedger();

    impl TryFromFeatureSet for AccountLedger {
        fn try_from_set(set: &FeatureSet) -> Result<Self, ManyError> {
            set.get_feature(ACCOUNT_LEDGER.id())
                .ok_or_else(|| ManyError::attribute_not_found(ACCOUNT_LEDGER.id().to_string()))
                .map(|_feature| Self())
        }
    }
}
