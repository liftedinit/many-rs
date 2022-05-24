use crate::server::module::EmptyReturn;
use crate::types::VecOrSingle;
use crate::{Identity, ManyError};
use many_macros::many_module;
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use std::collections::{BTreeMap, BTreeSet};
use strum_macros::{AsRefStr, EnumString};

pub mod errors;
pub mod features;

#[derive(
    Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, EnumString, AsRefStr, strum_macros::Display,
)]
#[repr(u8)]
#[strum(serialize_all = "camelCase")]
pub enum Role {
    Owner,
    CanLedgerSend,
    CanMultisigSubmit,
    CanMultisigApprove,
}

impl Encode for Role {
    fn encode<W: encode::Write>(&self, e: &mut Encoder<W>) -> Result<(), encode::Error<W::Error>> {
        e.str(self.as_ref())?;
        Ok(())
    }
}

impl<'b> Decode<'b> for Role {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, decode::Error> {
        let role = d.str()?;
        std::str::FromStr::from_str(role).map_err(|_| decode::Error::Message("Invalid role"))
    }
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

    pub fn contains(&self, identity: &Identity) -> bool {
        self.get(identity).is_some()
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

    pub fn remove(&mut self, identity: &Identity) -> Option<Account> {
        if identity.matches(&self.id) {
            if let Some(subid) = identity.subresource_id() {
                return self.inner.remove(&subid);
            }
        }
        None
    }

    pub fn has_role(&self, account: &Identity, id: &Identity, role: Role) -> bool {
        if let Some(account) = self.get(account) {
            account.has_role(id, role)
        } else {
            false
        }
    }

    pub fn iter(&self) -> AccountMapIterator {
        AccountMapIterator(self.id, self.inner.iter())
    }
}

/// A generic Account type. This is useful as utility for managing accounts in your backend.
#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct Account {
    #[n(0)]
    pub description: Option<String>,

    #[n(1)]
    pub roles: BTreeMap<Identity, BTreeSet<Role>>,

    #[n(2)]
    pub features: features::FeatureSet,
}

impl Account {
    pub fn create(
        sender: &Identity,
        CreateArgs {
            description,
            roles,
            features,
        }: CreateArgs,
    ) -> Self {
        // Add the sender as owner role.
        let mut roles = roles.unwrap_or_default();
        roles.entry(*sender).or_default().insert(Role::Owner);
        Self {
            description,
            roles,
            features,
        }
    }

    pub fn set_description(&mut self, desc: Option<impl ToString>) {
        self.description = desc.map(|d| d.to_string());
    }

    pub fn features(&self) -> &features::FeatureSet {
        &self.features
    }
    pub fn roles(&self) -> &BTreeMap<Identity, BTreeSet<Role>> {
        &self.roles
    }

    pub fn has_role<R: Into<Role>>(&self, id: &Identity, role: R) -> bool {
        self.roles
            .get(id)
            .map_or(false, |v| v.contains(&role.into()))
    }
    pub fn add_role<R: Into<Role>>(&mut self, id: &Identity, role: R) -> bool {
        self.roles.entry(*id).or_default().insert(role.into())
    }
    pub fn remove_role<R: Into<Role>>(&mut self, id: &Identity, role: R) -> bool {
        self.roles
            .get_mut(id)
            .map_or(false, |v| v.remove(&role.into()))
    }

    pub fn get_roles(&self, id: &Identity) -> BTreeSet<Role> {
        self.roles.get(id).cloned().unwrap_or_default()
    }

    pub fn feature<F: features::TryCreateFeature>(&self) -> Option<F> {
        self.features.get::<F>().ok()
    }
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct CreateArgs {
    #[n(0)]
    pub description: Option<String>,

    #[n(1)]
    pub roles: Option<BTreeMap<Identity, BTreeSet<Role>>>,

    #[n(2)]
    pub features: features::FeatureSet,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct CreateReturn {
    #[n(0)]
    pub id: Identity,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct SetDescriptionArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub description: String,
}

pub type SetDescriptionReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct ListRolesArgs {
    #[n(0)]
    pub account: Identity,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ListRolesReturn {
    #[n(0)]
    pub roles: BTreeSet<Role>,
}

#[derive(Clone, Debug, Encode, Decode)]
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
    pub roles: BTreeMap<Identity, BTreeSet<Role>>,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct AddRolesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub roles: BTreeMap<Identity, BTreeSet<Role>>,
}

pub type AddRolesReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct RemoveRolesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub roles: BTreeMap<Identity, BTreeSet<Role>>,
}

pub type RemoveRolesReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct InfoArgs {
    #[n(0)]
    pub account: Identity,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoReturn {
    #[n(0)]
    pub description: Option<String>,

    #[n(1)]
    pub roles: BTreeMap<Identity, BTreeSet<Role>>,

    #[n(2)]
    pub features: features::FeatureSet,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct DeleteArgs {
    #[n(0)]
    pub account: Identity,
}

pub type DeleteReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct AddFeaturesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub roles: Option<BTreeMap<Identity, BTreeSet<Role>>>,

    #[n(2)]
    pub features: features::FeatureSet,
}

pub type AddFeaturesReturn = EmptyReturn;

#[many_module(name = AccountModule, id = 9, namespace = account, many_crate = crate)]
#[cfg_attr(test, mockall::automock)]
pub trait AccountModuleBackend: Send {
    /// Create an account.
    fn create(&mut self, sender: &Identity, args: CreateArgs) -> Result<CreateReturn, ManyError>;

    /// Set the description of an account.
    fn set_description(
        &mut self,
        sender: &Identity,
        args: SetDescriptionArgs,
    ) -> Result<SetDescriptionReturn, ManyError>;

    /// List all the roles supported by an account.
    fn list_roles(
        &self,
        sender: &Identity,
        args: ListRolesArgs,
    ) -> Result<ListRolesReturn, ManyError>;

    /// Get roles associated with an identity for an account.
    fn get_roles(&self, sender: &Identity, args: GetRolesArgs)
        -> Result<GetRolesReturn, ManyError>;

    /// Add roles to identities for an account.
    fn add_roles(
        &mut self,
        sender: &Identity,
        args: AddRolesArgs,
    ) -> Result<AddRolesReturn, ManyError>;

    /// Remove roles from an identity for an account.
    fn remove_roles(
        &mut self,
        sender: &Identity,
        args: RemoveRolesArgs,
    ) -> Result<RemoveRolesReturn, ManyError>;

    /// Returns the information related to an account.
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturn, ManyError>;

    /// Delete an account.
    fn delete(&mut self, sender: &Identity, args: DeleteArgs) -> Result<DeleteReturn, ManyError>;

    /// Add additional features to an account.
    fn add_features(
        &mut self,
        sender: &Identity,
        args: AddFeaturesArgs,
    ) -> Result<AddFeaturesReturn, ManyError>;
}

#[cfg(test)]
mod module_tests {
    use super::*;
    use crate::server::module::testutils::call_module;
    use crate::types::identity::tests;
    use std::sync::{Arc, Mutex, RwLock};

    // TODO: split this to get easier to maintain tests.
    #[test]
    fn module_works() {
        let account_map = Arc::new(RwLock::new(AccountMap::new(Identity::public_key_raw(
            [0; 28],
        ))));
        let mut mock = MockAccountModuleBackend::new();

        mock.expect_create().returning({
            let account_map = Arc::clone(&account_map);
            move |sender, args| {
                let mut account_map = account_map.write().unwrap();
                let (id, _) = account_map.insert(Account::create(sender, args))?;
                Ok(CreateReturn { id })
            }
        });
        mock.expect_set_description().returning({
            let account_map = Arc::clone(&account_map);
            move |_, args| {
                let mut account_map = account_map.write().unwrap();
                let mut account = account_map
                    .get_mut(&args.account)
                    .ok_or_else(|| errors::unknown_account(args.account))?;
                account.description = Some(args.description);
                Ok(EmptyReturn)
            }
        });
        mock.expect_info().returning({
            let account_map = Arc::clone(&account_map);
            move |_, args| {
                let account_map = account_map.write().unwrap();
                let account = account_map
                    .get(&args.account)
                    .ok_or_else(|| errors::unknown_account(args.account))?;
                Ok(InfoReturn {
                    description: account.description.clone(),
                    roles: account.roles.clone(),
                    features: account.features.clone(),
                })
            }
        });
        mock.expect_list_roles().returning({
            let account_map = Arc::clone(&account_map);
            move |_, args| {
                let account_map = account_map.write().unwrap();
                let _ = account_map
                    .get(&args.account)
                    .ok_or_else(|| errors::unknown_account(args.account))?;

                Ok(ListRolesReturn {
                    roles: BTreeSet::from_iter(vec![Role::Owner, Role::CanLedgerSend].into_iter()),
                })
            }
        });
        mock.expect_delete().returning({
            let account_map = Arc::clone(&account_map);
            move |sender, args| {
                let mut account_map = account_map.write().unwrap();
                if account_map.has_role(&args.account, sender, Role::Owner) {
                    account_map.remove(&args.account).map_or_else(
                        || Err(errors::unknown_account(args.account)),
                        |_| Ok(EmptyReturn),
                    )
                } else {
                    Err(errors::user_needs_role(Role::Owner))
                }
            }
        });

        let module_impl = Arc::new(Mutex::new(mock));
        let module = super::AccountModule::new(module_impl.clone());
        let id_from = tests::identity(1);

        let result: CreateReturn = minicbor::decode(
            &call_module(1, &module, "account.create", r#"{ 0: "test", 2: [0] }"#).unwrap(),
        )
        .unwrap();

        let id = {
            let account_map = account_map.read().unwrap();
            let (id, account) = account_map.iter().next().unwrap();

            assert_eq!(id, result.id);
            assert_eq!(id.subresource_id(), Some(0));
            assert_eq!(account.description, Some("test".to_string()));
            assert!(account.roles.contains_key(&id_from));
            assert!(account
                .roles
                .get_key_value(&id_from)
                .unwrap()
                .1
                .contains(&Role::Owner));
            id
        };
        let wrong_id = id.with_subresource_id(12345).unwrap();

        call_module(
            1,
            &module,
            "account.setDescription",
            format!(r#"{{ 0: "{}", 1: "new-name" }}"#, id),
        )
        .unwrap();

        assert!(call_module(
            1,
            &module,
            "account.setDescription",
            format!(r#"{{ 0: "{}", 1: "new-name-2" }}"#, wrong_id),
        )
        .is_err());

        {
            let account: InfoReturn = minicbor::decode(
                &call_module(0, &module, "account.info", format!(r#"{{ 0: "{}" }}"#, id)).unwrap(),
            )
            .unwrap();
            assert_eq!(account.description, Some("new-name".to_string()));
            assert!(account.roles.contains_key(&id_from));
            assert!(account
                .roles
                .get_key_value(&id_from)
                .unwrap()
                .1
                .contains(&Role::Owner));
            assert!(account.features.has_id(0));
        }

        assert!(call_module(
            1,
            &module,
            "account.listRoles",
            format!(r#"{{ 0: "{}", 1: "new-name" }}"#, wrong_id),
        )
        .is_err());

        assert_eq!(
            minicbor::decode::<ListRolesReturn>(
                &call_module(
                    1,
                    &module,
                    "account.listRoles",
                    format!(r#"{{ 0: "{}", 1: "new-name" }}"#, id)
                )
                .unwrap()
            )
            .unwrap()
            .roles,
            BTreeSet::from_iter(vec![Role::Owner, Role::CanLedgerSend].into_iter()),
        );

        assert!(call_module(
            2,
            &module,
            "account.delete",
            format!(r#"{{ 0: "{}" }}"#, id),
        )
        .is_err());
        assert!(call_module(
            1,
            &module,
            "account.delete",
            format!(r#"{{ 0: "{}" }}"#, id.with_subresource_id(9999).unwrap()),
        )
        .is_err());

        assert!(call_module(
            1,
            &module,
            "account.delete",
            format!(r#"{{ 0: "{}" }}"#, id),
        )
        .is_ok());

        let account_map = account_map.read().unwrap();
        assert!(account_map.inner.is_empty());
    }
}
