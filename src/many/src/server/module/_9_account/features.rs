use crate::cbor::CborAny;
use crate::protocol::Attribute;
use crate::ManyError;
use minicbor::{Decode, Encode};
use std::collections::BTreeSet;

/// See feature `_0_account_ledger`.
pub mod ledger {
    use super::{Feature, FeatureId, TryCreateFeature};
    use crate::ManyError;

    pub struct AccountLedger;

    impl TryCreateFeature for AccountLedger {
        const ID: FeatureId = 0;

        fn try_create(_: &Feature) -> Result<Self, ManyError> {
            Ok(Self)
        }
    }
}

pub type FeatureId = u32;

/// An Account Feature.
#[derive(Encode, Decode, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
#[cbor(transparent)]
pub struct Feature(#[n(0)] Attribute);

impl Feature {
    /// Create a feature with a specific ID.
    pub const fn with_id(id: FeatureId) -> Self {
        Self(Attribute::id(id))
    }

    pub fn new(id: FeatureId, arguments: Vec<CborAny>) -> Self {
        Self(Attribute::new(id, arguments))
    }

    pub const fn id(&self) -> FeatureId {
        self.0.id
    }

    pub fn with_argument(&self, argument: CborAny) -> Self {
        Self(self.0.with_argument(argument))
    }

    pub fn arguments(&self) -> &Vec<CborAny> {
        self.0.arguments()
    }
}

/// A set of features related to a specific account.
///
/// ```
/// # use many::cbor::CborAny;
/// # use many::server::module::account::features::*;
/// let mut feature_set = FeatureSet::default();
/// feature_set.insert(Feature::with_id(0));
/// feature_set.insert(Feature::with_id(1).with_arguments(vec![CborAny::Int(123)]));
///
/// assert!(feature_set.get_feature(0).is_some());
/// assert!(feature_set.get_feature(1).is_some());
/// assert!(feature_set.get_feature(2).is_none());
/// ```
#[derive(Encode, Decode, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq)]
#[cbor(transparent)]
pub struct FeatureSet(#[n(0)] BTreeSet<Feature>);

impl FeatureSet {
    /// Returns true if the set is empty.
    ///
    /// ```
    /// # use many::server::module::account::features::{Feature, FeatureSet};
    /// let mut set = FeatureSet::default();
    /// assert!(set.is_empty());
    /// set.insert(Feature::with_id(0));
    /// assert!(!set.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert(&mut self, attr: Feature) -> bool {
        self.0.insert(attr)
    }

    pub fn remove(&mut self, id: FeatureId) -> bool {
        self.0.remove(&Feature::with_id(id))
    }

    pub fn has_id(&self, id: FeatureId) -> bool {
        self.0.iter().any(|a| id == a.id())
    }

    pub fn contains(&self, a: &Feature) -> bool {
        self.0.contains(a)
    }

    pub fn get_feature(&self, id: FeatureId) -> Option<&Feature> {
        self.0.iter().find(|a| id == a.id())
    }

    /// Get a feature's wrapper class.
    pub fn get<T: TryCreateFeature>(&self) -> Result<T, ManyError> {
        self.get_feature(T::ID).map_or_else(
            || Err(ManyError::attribute_not_found(format!("{}", T::ID))),
            |f| T::try_create(f),
        )
    }

    /// Creates an iterator to traverse all features.
    pub fn iter(&self) -> impl Iterator<Item = &Feature> {
        self.0.iter()
    }
}

pub trait TryCreateFeature: Sized {
    const ID: FeatureId;

    fn try_create(feature: &Feature) -> Result<Self, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features() {
        let mut set = FeatureSet::default();
        set.insert(Feature::with_id(0));
        set.insert(Feature::with_id(5).with_argument(CborAny::Int(1)));

        struct SomeFeature;
        impl TryCreateFeature for SomeFeature {
            const ID: FeatureId = 1;
            fn try_create(f: &Feature) -> Result<Self, ManyError> {
                match f.arguments().as_slice() {
                    &[CborAny::Int(123)] => Ok(Self),
                    _ => Err(ManyError::unknown("ERROR".to_string())),
                }
            }
        }

        assert!(set.get::<SomeFeature>().is_err());

        set.insert(Feature::with_id(1));
        assert!(set.get::<SomeFeature>().is_err());

        set.remove(1);
        set.insert(Feature::with_id(1).with_argument(CborAny::Int(2)));
        assert!(set.get::<SomeFeature>().is_err());

        set.remove(1);
        set.insert(Feature::with_id(1).with_argument(CborAny::Int(123)));
        assert!(set.get::<SomeFeature>().is_ok());

        assert_eq!(
            Vec::from_iter(set.iter()).as_slice(),
            &[
                &Feature::with_id(0),
                &Feature::with_id(1).with_argument(CborAny::Int(123)),
                &Feature::with_id(5).with_argument(CborAny::Int(1)),
            ]
        );
    }
}
