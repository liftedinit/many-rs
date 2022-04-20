use crate::server::module::EmptyReturn;
use crate::types::VecOrSingle;
use crate::{Identity, ManyError};
use many_macros::many_module;
use minicbor::{Decode, Encode};
use std::collections::{BTreeMap, BTreeSet};

pub mod errors;
pub mod features;

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

/// A map of Subresource IDs to account. It should have a non-anonymous identity as the identity,
/// and the inner map will contains subresource identities as keys.
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

    pub fn get(&self, identity: &Identity) -> Option<&Account> {
        if identity.matches(&self.id) {
            if let Some(subid) = identity.subresource_id() {
                return self.inner.get(&subid);
            }
        }
        None
    }

    pub fn get_mut(&mut self, identity: &Identity) -> Option<&mut Account> {
        if identity.matches(&self.id) {
            if let Some(subid) = identity.subresource_id() {
                return self.inner.get_mut(&subid);
            }
        }
        None
    }

    pub fn insert(&mut self, account: Account) -> Result<(Identity, Option<Account>), ManyError> {
        let subid = self.inner.keys().last().map_or(0, |x| x + 1);
        let id = self.id.with_subresource_id(subid)?;
        Ok((id, self.inner.insert(subid, account)))
    }

    pub fn iter(&self) -> AccountMapIterator {
        AccountMapIterator(self.id, self.inner.iter())
    }
}

/// A generic Account type. This is useful as utility for managing accounts in your backend.
#[derive(Clone, Debug)]
pub struct Account {
    name: String,
    roles: BTreeMap<Identity, BTreeSet<String>>,
    features: features::FeatureSet,
}

impl Account {
    pub fn create(args: CreateArgs) -> Self {
        Self {
            name: args.description.unwrap_or_default(),
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
    pub fn feature<F: features::TryCreateFeature>(&self) -> Option<F> {
        self.features.get::<F>().ok()
    }
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct CreateArgs {
    #[n(0)]
    pub description: Option<String>,

    #[n(1)]
    pub roles: Option<BTreeMap<Identity, BTreeSet<String>>>,

    #[n(2)]
    pub features: features::FeatureSet,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct CreateReturn {
    #[n(0)]
    pub id: Identity,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct SetDescriptionArgs {
    #[n(0)]
    pub id: Identity,

    #[n(1)]
    pub description: String,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ListRolesArgs {
    #[n(0)]
    pub account: Identity,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ListRolesReturn {
    #[n(0)]
    roles: Vec<String>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetRolesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub identities: VecOrSingle<Identity>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct GetRolesReturn {
    #[n(0)]
    roles: BTreeMap<Identity, Vec<String>>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct AddRolesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub roles: BTreeMap<Identity, Vec<String>>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct RemoveRolesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub roles: BTreeMap<Identity, Vec<String>>,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoArgs {
    #[n(0)]
    pub account: Identity,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoReturn {
    #[n(0)]
    name: String,

    #[n(1)]
    roles: BTreeMap<Identity, BTreeSet<String>>,

    #[n(2)]
    features: features::FeatureSet,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct DeleteArgs {
    #[n(0)]
    pub account: Identity,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct AddFeaturesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub features: features::FeatureSet,
}

#[many_module(name = AccountModule, id = 9, namespace = account, many_crate = crate)]
pub trait AccountModuleBackend: Send {
    fn create(&mut self, sender: &Identity, args: CreateArgs) -> Result<CreateReturn, ManyError>;
    fn set_description(
        &mut self,
        sender: &Identity,
        args: SetDescriptionArgs,
    ) -> Result<EmptyReturn, ManyError>;
    fn list_roles(
        &self,
        sender: &Identity,
        args: ListRolesArgs,
    ) -> Result<ListRolesReturn, ManyError>;
    fn get_roles(&self, sender: &Identity, args: GetRolesArgs)
        -> Result<GetRolesReturn, ManyError>;
    fn add_roles(&self, sender: &Identity, args: AddRolesArgs) -> Result<EmptyReturn, ManyError>;
    fn remove_roles(
        &self,
        sender: &Identity,
        args: RemoveRolesArgs,
    ) -> Result<EmptyReturn, ManyError>;
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturn, ManyError>;
    fn delete(&self, sender: &Identity, args: DeleteArgs) -> Result<EmptyReturn, ManyError>;
    fn add_features(
        &self,
        sender: &Identity,
        args: AddFeaturesArgs,
    ) -> Result<EmptyReturn, ManyError>;
}

#[cfg(test)]
mod module_tests {
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

        fn set_description(
            &mut self,
            _sender: &Identity,
            args: SetDescriptionArgs,
        ) -> Result<EmptyReturn, ManyError> {
            let mut account = self
                .0
                .get_mut(&args.id)
                .ok_or_else(|| errors::unknown_account(args.id.to_string()))?;

            account.name = args.description;
            Ok(EmptyReturn)
        }

        fn list_roles(
            &self,
            _sender: &Identity,
            args: ListRolesArgs,
        ) -> Result<ListRolesReturn, ManyError> {
            let account = self
                .0
                .get(&args.account)
                .ok_or_else(|| errors::unknown_account(args.account.to_string()))?;

            Ok(ListRolesReturn {
                roles: vec!["owner".to_string(), "other-role".to_string()],
            })
        }
    }

    #[test]
    fn module_works() {
        let module_impl = Arc::new(Mutex::new(AccountImpl::default()));
        let module = super::AccountModule::new(module_impl.clone());

        let result: CreateReturn = minicbor::decode(
            &call_module(&module, "account.create", r#"{ 0: "test", 2: [0] }"#).unwrap(),
        )
        .unwrap();

        let id = {
            let lock = module_impl.lock().unwrap();
            let (id, account) = lock.0.iter().next().unwrap();

            assert_eq!(id, result.id);
            assert_eq!(id.subresource_id(), Some(0));
            assert_eq!(account.name, "test");
            id
        };
        let wrong_id = id.with_subresource_id(12345).unwrap();

        call_module(
            &module,
            "account.setDescription",
            format!(r#"{{ 0: "{}", 1: "new-name" }}"#, id),
        )
        .unwrap();

        assert!(call_module(
            &module,
            "account.setDescription",
            format!(r#"{{ 0: "{}", 1: "new-name" }}"#, wrong_id),
        )
        .is_err());

        {
            let lock = module_impl.lock().unwrap();
            let account = lock.0.get(&id).unwrap();
            assert_eq!(account.name, "new-name");
        }

        assert!(call_module(
            &module,
            "account.listRoles",
            format!(r#"{{ 0: "{}", 1: "new-name" }}"#, wrong_id),
        )
        .is_err());
        assert_eq!(
            minicbor::decode::<ListRolesReturn>(
                &call_module(
                    &module,
                    "account.listRoles",
                    format!(r#"{{ 0: "{}", 1: "new-name" }}"#, id)
                )
                .unwrap()
            )
            .unwrap()
            .roles,
            vec!["owner", "other-role"]
        );
    }
}
