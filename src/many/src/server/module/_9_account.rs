use crate::cbor::CborAny;
use crate::protocol::Attribute;
use crate::{Identity, ManyError};
use many_macros::many_module;
use minicbor::{Decode, Encode};
use std::collections::{BTreeMap, BTreeSet};

pub mod features;

pub type FeatureId = u32;

/// An Account Feature.
#[derive(Encode, Decode, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
#[cbor(transparent)]
pub struct Feature(#[n(0)] Attribute);

impl Feature {
    pub const fn with_id(id: FeatureId) -> Self {
        Self(Attribute::id(id))
    }

    pub fn new(id: FeatureId, arguments: Vec<CborAny>) -> Self {
        Self(Attribute::new(id, arguments))
    }

    pub fn with_arguments(&self, arguments: Vec<CborAny>) -> Self {
        Self(self.0.with_arguments(arguments))
    }

    pub const fn id(&self) -> FeatureId {
        self.0.id
    }

    pub fn arguments(&self) -> Option<&Vec<CborAny>> {
        self.0.arguments()
    }
}

#[derive(Encode, Decode, Clone, Debug, Default, Ord, PartialOrd, Eq, PartialEq)]
#[cbor(transparent)]
pub struct FeatureSet(#[n(0)] BTreeSet<Feature>);

impl FeatureSet {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert(&mut self, attr: Feature) -> bool {
        self.0.insert(attr)
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

    pub fn get<T: TryFromFeatureSet>(&self) -> Result<T, ManyError> {
        TryFromFeatureSet::try_from_set(self)
    }

    pub fn iter(&self) -> std::collections::btree_set::Iter<Feature> {
        self.0.iter()
    }
}

pub trait TryFromFeatureSet: Sized {
    fn try_from_set(set: &FeatureSet) -> Result<Self, ManyError>;
}

/// An iterator that iterates over accounts. The keys will be identities, and not just
/// subresource IDs.
#[derive(Clone)]
pub struct AccountMapIterator<'map>(
    Identity,
    std::collections::btree_map::Iter<'map, u32, Account>,
);

impl std::fmt::Debug for AccountMapIterator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<'it> Iterator for AccountMapIterator<'it> {
    type Item = (Identity, &'it Account);

    fn next(&mut self) -> Option<Self::Item> {
        self.1
            .next()
            .map(|(k, v)| (self.0.with_subresource_id_unchecked(*k), v))
    }
}

/// This is a map of accounts. It should have a non-anonymous identity as the identity,
/// and the inner map will contains subresource identities.
pub struct AccountMap {
    id: Identity,
    inner: BTreeMap<u32, Account>,
}

impl AccountMap {
    pub fn new(id: Identity) -> Self {
        Self {
            id,
            inner: Default::default(),
        }
    }

    pub fn insert(&mut self, account: Account) -> Result<(Identity, Option<Account>), ManyError> {
        let subid = self.inner.keys().max().map_or(0, |x| x + 1);
        let id = self.id.with_subresource_id(subid)?;
        Ok((id, self.inner.insert(subid, account)))
    }

    pub fn iter(&self) -> AccountMapIterator {
        AccountMapIterator(self.id.clone(), self.inner.iter())
    }
}

/// A generic Account type. This is useful as utility for managing accounts in your backend.
#[derive(Clone, Debug)]
pub struct Account {
    name: String,
    roles: BTreeMap<Identity, BTreeSet<String>>,
    features: FeatureSet,
}

impl Account {
    pub fn create(args: CreateArgs) -> Self {
        Self {
            name: args.name.unwrap_or_default(),
            roles: args.roles.unwrap_or_default(),
            features: args.features,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn roles(&self) -> &BTreeMap<Identity, BTreeSet<String>> {
        &self.roles
    }
    pub fn has_role<Role: AsRef<str>>(&self, id: &Identity, role: Role) -> bool {
        self.roles
            .get(id)
            .map_or(false, |v| v.contains(role.as_ref()))
    }
    pub fn feature<F: TryFromFeatureSet>(&self) -> Option<F> {
        self.features.get::<F>().ok()
    }
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct CreateArgs {
    #[n(0)]
    name: Option<String>,

    #[n(1)]
    roles: Option<BTreeMap<Identity, BTreeSet<String>>>,

    #[n(2)]
    features: FeatureSet,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct CreateReturn {
    #[n(0)]
    id: Identity,
}

#[many_module(name = AccountModule, id = 9, namespace = account, many_crate = crate)]
pub trait AccountModuleBackend: Send {
    fn create(&mut self, sender: &Identity, args: CreateArgs) -> Result<CreateReturn, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::module::testutils::call_module;
    use std::sync::{Arc, Mutex};

    struct AccountImpl(pub AccountMap);
    impl std::default::Default for AccountImpl {
        fn default() -> Self {
            Self(AccountMap::new(
                Identity::from_bytes(
                    &hex::decode("0102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D")
                        .unwrap(),
                )
                .unwrap(),
            ))
        }
    }

    impl super::AccountModuleBackend for AccountImpl {
        fn create(
            &mut self,
            _sender: &Identity,
            args: CreateArgs,
        ) -> Result<CreateReturn, ManyError> {
            let (id, _) = self.0.insert(Account::create(args))?;
            Ok(CreateReturn { id })
        }
    }

    #[test]
    fn module_works() {
        let module_impl = Arc::new(Mutex::new(AccountImpl::default()));
        let module = super::AccountModule::new(module_impl.clone());

        let result: CreateReturn = minicbor::decode(
            &call_module(module, "account.create", r#"{ 0: "test", 2: [0] }"#).unwrap(),
        )
        .unwrap();

        let lock = module_impl.lock().unwrap();
        let (id, _account) = lock.0.iter().next().unwrap();

        assert_eq!(id, result.id);
        assert_eq!(id.subresource_id(), Some(0));
    }
}
