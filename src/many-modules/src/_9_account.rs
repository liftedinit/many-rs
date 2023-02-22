use crate::events::AddressContainer;
use crate::EmptyReturn;
use many_error::{ManyError, Reason};
use many_identity::Address;
use many_macros::many_module;
use many_protocol::context::Context;
use many_types::{Either, VecOrSingle};
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

pub mod errors;
pub mod features;

#[derive(
    Copy,
    Clone,
    Debug,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    strum_macros::AsRefStr,
    strum_macros::Display,
    strum_macros::EnumIter,
    strum_macros::EnumString,
)]
#[repr(u8)]
#[strum(serialize_all = "camelCase")]
pub enum Role {
    Owner,
    CanLedgerTransact,
    CanMultisigSubmit,
    CanMultisigApprove,
    CanKvStorePut,
    CanKvStoreDisable,
    CanKvStoreTransfer,
    CanTokensCreate,
    CanTokensMint,
    CanTokensBurn,
    CanTokensUpdate,
    CanTokensAddExtendedInfo,
    CanTokensRemoveExtendedInfo,
}

impl PartialEq<&str> for Role {
    fn eq(&self, other: &&str) -> bool {
        self.as_ref() == *other
    }
}

impl<C> Encode<C> for Role {
    fn encode<W: encode::Write>(
        &self,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), encode::Error<W::Error>> {
        e.str(self.as_ref())?;
        Ok(())
    }
}

impl<'b, C> Decode<'b, C> for Role {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        let role = d.str()?;
        Self::from_str(role).map_err(|_| decode::Error::message("Invalid role"))
    }
}

/// An iterator that iterates over accounts. The keys will be identities, and not just
/// subresource IDs.
#[derive(Clone)]
pub struct AccountMapIterator<'map>(
    Address,
    std::collections::btree_map::Iter<'map, u32, Account>,
);

impl std::fmt::Debug for AccountMapIterator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<'it> Iterator for AccountMapIterator<'it> {
    type Item = (Address, &'it Account);

    fn next(&mut self) -> Option<Self::Item> {
        self.1
            .next()
            .map(|(k, v)| (self.0.with_subresource_id(*k).unwrap(), v))
    }
}

pub type AddressRoleMap = BTreeMap<Address, BTreeSet<Role>>;

/// A map of Subresource IDs to account. It should have a non-anonymous identity as the identity,
/// and the inner map will contains subresource identities as keys.
pub struct AccountMap {
    id: Address,
    inner: BTreeMap<u32, Account>,
}

impl AccountMap {
    pub fn new(id: Address) -> Self {
        Self {
            id,
            inner: Default::default(),
        }
    }

    pub fn contains(&self, identity: &Address) -> bool {
        self.get(identity).is_some()
    }

    pub fn get(&self, identity: &Address) -> Option<&Account> {
        if identity.matches(&self.id) {
            if let Some(subid) = identity.subresource_id() {
                return self.inner.get(&subid);
            }
        }
        None
    }

    pub fn get_mut(&mut self, identity: &Address) -> Option<&mut Account> {
        if identity.matches(&self.id) {
            if let Some(subid) = identity.subresource_id() {
                return self.inner.get_mut(&subid);
            }
        }
        None
    }

    pub fn insert(&mut self, account: Account) -> Result<(Address, Option<Account>), ManyError> {
        let subid = self.inner.keys().last().map_or(0, |x| x + 1);
        let id = self.id.with_subresource_id(subid)?;
        Ok((id, self.inner.insert(subid, account)))
    }

    pub fn remove(&mut self, identity: &Address) -> Option<Account> {
        if identity.matches(&self.id) {
            if let Some(subid) = identity.subresource_id() {
                return self.inner.remove(&subid);
            }
        }
        None
    }

    pub fn has_role(&self, account: &Address, id: &Address, role: Role) -> bool {
        if let Some(account) = self.get(account) {
            account.has_role(id, role)
        } else {
            false
        }
    }

    pub fn needs_role(
        &self,
        account: &Address,
        id: &Address,
        role: impl IntoIterator<Item = Role>,
    ) -> Result<(), ManyError> {
        if let Some(account) = self.get(account) {
            account.needs_role(id, role)
        } else {
            Err(errors::unknown_account(account))
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
    pub roles: AddressRoleMap,

    #[n(2)]
    pub features: features::FeatureSet,

    #[n(3)]
    pub disabled: Option<Either<bool, Reason<u64>>>,
}

impl Account {
    pub fn create(
        sender: &Address,
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
            disabled: None,
        }
    }

    /// Disable the account, providing a reason or not.
    pub fn disable(&mut self, reason: Option<Reason<u64>>) {
        self.disabled = Some(match reason {
            None => Either::Left(true),
            Some(e) => Either::Right(e),
        })
    }

    pub fn set_description(&mut self, desc: Option<impl ToString>) {
        self.description = desc.map(|d| d.to_string());
    }

    pub fn features(&self) -> &features::FeatureSet {
        &self.features
    }
    pub fn roles(&self) -> &AddressRoleMap {
        &self.roles
    }

    pub fn has_role<R: TryInto<Role>>(&self, id: &Address, role: R) -> bool {
        let role: Role = if let Ok(r) = role.try_into() {
            r
        } else {
            return false;
        };
        self.roles.get(id).map_or(false, |v| v.contains(&role))
    }

    /// Verify that an ID has the proper role, or return an
    pub fn needs_role<R: TryInto<Role> + std::fmt::Display + Copy>(
        &self,
        id: &Address,
        role: impl IntoIterator<Item = R>,
    ) -> Result<(), ManyError> {
        let mut first = None;
        for role in role {
            let cp = role;
            match role.try_into() {
                Ok(r) => {
                    first.get_or_insert(r);
                    if self.has_role(id, r) {
                        return Ok(());
                    }
                }
                Err(_) => return Err(errors::unknown_role(cp)),
            }
        }
        Err(errors::user_needs_role(first.unwrap_or(Role::Owner)))
    }

    pub fn add_role<R: Into<Role>>(&mut self, id: &Address, role: R) -> bool {
        self.roles.entry(*id).or_default().insert(role.into())
    }

    pub fn remove_role<R: Into<Role>>(&mut self, id: &Address, role: R) -> bool {
        let v = self.roles.get_mut(id);
        match v {
            Some(v) => {
                let result = v.remove(&role.into());
                if v.is_empty() {
                    self.roles.remove(id);
                }
                result
            }
            None => false,
        }
    }

    pub fn get_roles(&self, id: &Address) -> BTreeSet<Role> {
        self.roles.get(id).cloned().unwrap_or_default()
    }

    pub fn feature<F: features::TryCreateFeature>(&self) -> Option<F> {
        self.features.get::<F>().ok()
    }
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct CreateArgs {
    #[n(0)]
    pub description: Option<String>,

    #[n(1)]
    pub roles: Option<AddressRoleMap>,

    #[n(2)]
    pub features: features::FeatureSet,
}

impl AddressContainer for CreateArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        self.roles.addresses()
    }
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct CreateReturn {
    #[n(0)]
    pub id: Address,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct SetDescriptionArgs {
    #[n(0)]
    pub account: Address,

    #[n(1)]
    pub description: String,
}

impl AddressContainer for SetDescriptionArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        BTreeSet::from([self.account])
    }
}

pub type SetDescriptionReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct ListRolesArgs {
    #[n(0)]
    pub account: Address,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ListRolesReturn {
    #[n(0)]
    pub roles: BTreeSet<Role>,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct GetRolesArgs {
    #[n(0)]
    pub account: Address,

    #[n(1)]
    pub identities: VecOrSingle<Address>,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct GetRolesReturn {
    #[n(0)]
    pub roles: AddressRoleMap,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct AddRolesArgs {
    #[n(0)]
    pub account: Address,

    #[n(1)]
    pub roles: AddressRoleMap,
}

impl AddressContainer for AddRolesArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        let mut set = BTreeSet::from([self.account]);
        set.extend(self.roles.addresses());
        set
    }
}

pub type AddRolesReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct RemoveRolesArgs {
    #[n(0)]
    pub account: Address,

    #[n(1)]
    pub roles: AddressRoleMap,
}

impl AddressContainer for RemoveRolesArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        let mut set = BTreeSet::from([self.account]);
        set.extend(self.roles.addresses());
        set
    }
}

pub type RemoveRolesReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct InfoArgs {
    #[n(0)]
    pub account: Address,
}

#[derive(Clone, Debug, Encode, Decode)]
#[cbor(map)]
pub struct InfoReturn {
    #[n(0)]
    pub description: Option<String>,

    #[n(1)]
    pub roles: AddressRoleMap,

    #[n(2)]
    pub features: features::FeatureSet,

    #[n(3)]
    pub disabled: Option<Either<bool, Reason<u64>>>,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct DisableArgs {
    #[n(0)]
    pub account: Address,
}

impl AddressContainer for DisableArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        BTreeSet::from([self.account])
    }
}

pub type DisableReturn = EmptyReturn;

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct AddFeaturesArgs {
    #[n(0)]
    pub account: Address,

    #[n(1)]
    pub roles: Option<AddressRoleMap>,

    #[n(2)]
    pub features: features::FeatureSet,
}

impl AddressContainer for AddFeaturesArgs {
    fn addresses(&self) -> BTreeSet<Address> {
        let mut set = BTreeSet::from([self.account]);
        set.extend(self.roles.addresses());
        set
    }
}

pub type AddFeaturesReturn = EmptyReturn;

#[many_module(name = AccountModule, id = 9, namespace = account, many_modules_crate = crate)]
#[cfg_attr(test, mockall::automock)]
pub trait AccountModuleBackend: Send {
    /// Create an account.
    fn create(&mut self, sender: &Address, args: CreateArgs) -> Result<CreateReturn, ManyError>;

    /// Set the description of an account.
    fn set_description(
        &mut self,
        sender: &Address,
        args: SetDescriptionArgs,
    ) -> Result<SetDescriptionReturn, ManyError>;

    /// List all the roles supported by an account.
    fn list_roles(
        &self,
        sender: &Address,
        args: ListRolesArgs,
        context: Context,
    ) -> Result<ListRolesReturn, ManyError>;

    /// Get roles associated with an identity for an account.
    fn get_roles(
        &self,
        sender: &Address,
        args: GetRolesArgs,
        context: Context,
    ) -> Result<GetRolesReturn, ManyError>;

    /// Add roles to identities for an account.
    fn add_roles(
        &mut self,
        sender: &Address,
        args: AddRolesArgs,
    ) -> Result<AddRolesReturn, ManyError>;

    /// Remove roles from an identity for an account.
    fn remove_roles(
        &mut self,
        sender: &Address,
        args: RemoveRolesArgs,
    ) -> Result<RemoveRolesReturn, ManyError>;

    /// Returns the information related to an account.
    fn info(
        &self,
        sender: &Address,
        args: InfoArgs,
        context: Context,
    ) -> Result<InfoReturn, ManyError>;

    /// Disable or delete an account.
    fn disable(&mut self, sender: &Address, args: DisableArgs) -> Result<DisableReturn, ManyError>;

    /// Add additional features to an account.
    fn add_features(
        &mut self,
        sender: &Address,
        args: AddFeaturesArgs,
    ) -> Result<AddFeaturesReturn, ManyError>;
}

#[cfg(test)]
mod module_tests {
    use super::*;
    use crate::testutils::call_module;
    use many_identity::testing::identity;
    use std::sync::{Arc, Mutex, RwLock};

    // TODO: split this to get easier to maintain tests.
    #[test]
    fn module_works() {
        let account_map = Arc::new(RwLock::new(AccountMap::new(identity(0))));
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
            move |_, args, _| {
                let account_map = account_map.write().unwrap();
                let account = account_map
                    .get(&args.account)
                    .ok_or_else(|| errors::unknown_account(args.account))?;
                Ok(InfoReturn {
                    description: account.description.clone(),
                    roles: account.roles.clone(),
                    features: account.features.clone(),
                    disabled: None,
                })
            }
        });
        mock.expect_list_roles().returning({
            let account_map = Arc::clone(&account_map);
            move |_, args, _| {
                let account_map = account_map.write().unwrap();
                let _ = account_map
                    .get(&args.account)
                    .ok_or_else(|| errors::unknown_account(args.account))?;

                Ok(ListRolesReturn {
                    roles: BTreeSet::from_iter(
                        vec![Role::Owner, Role::CanLedgerTransact].into_iter(),
                    ),
                })
            }
        });
        mock.expect_disable().returning({
            let account_map = Arc::clone(&account_map);
            move |sender, args| {
                let mut account_map = account_map.write().unwrap();
                account_map.needs_role(&args.account, sender, [Role::Owner])?;
                account_map.remove(&args.account).map_or_else(
                    || Err(errors::unknown_account(args.account)),
                    |_| Ok(EmptyReturn),
                )
            }
        });

        let module_impl = Arc::new(Mutex::new(mock));
        let module = super::AccountModule::new(module_impl);
        let id_from = identity(1);

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
            format!(r#"{{ 0: "{id}", 1: "new-name" }}"#),
        )
        .unwrap();

        assert!(call_module(
            1,
            &module,
            "account.setDescription",
            format!(r#"{{ 0: "{wrong_id}", 1: "new-name-2" }}"#),
        )
        .is_err());

        {
            let account: InfoReturn = minicbor::decode(
                &call_module(0, &module, "account.info", format!(r#"{{ 0: "{id}" }}"#)).unwrap(),
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
            format!(r#"{{ 0: "{wrong_id}", 1: "new-name" }}"#),
        )
        .is_err());

        assert_eq!(
            minicbor::decode::<ListRolesReturn>(
                &call_module(
                    1,
                    &module,
                    "account.listRoles",
                    format!(r#"{{ 0: "{id}", 1: "new-name" }}"#)
                )
                .unwrap()
            )
            .unwrap()
            .roles,
            BTreeSet::from_iter(vec![Role::Owner, Role::CanLedgerTransact].into_iter()),
        );

        assert!(
            call_module(2, &module, "account.disable", format!(r#"{{ 0: "{id}" }}"#),).is_err()
        );
        assert!(call_module(
            1,
            &module,
            "account.disable",
            format!(r#"{{ 0: "{}" }}"#, id.with_subresource_id(9999).unwrap()),
        )
        .is_err());

        assert!(call_module(1, &module, "account.disable", format!(r#"{{ 0: "{id}" }}"#),).is_ok());

        let account_map = account_map.read().unwrap();
        assert!(account_map.inner.is_empty());
    }
}

#[test]
fn roles_from_str() {
    use std::str::FromStr;
    assert_eq!(Role::from_str("owner").unwrap(), Role::Owner);
    assert_eq!(Role::Owner, "owner");
    assert_eq!(format!("a {} b", Role::Owner), "a owner b");
}

#[test]
fn needs_role() {
    use many_identity::testing::identity;

    let owner = identity(0);
    let account = Account::create(
        &owner,
        CreateArgs {
            description: None,
            roles: None,
            features: Default::default(),
        },
    );
    assert!(account.needs_role(&owner, [Role::Owner]).is_ok());
    assert!(account
        .needs_role(&owner, [Role::CanMultisigSubmit])
        .is_err());
    assert!(account.needs_role(&identity(1), [Role::Owner]).is_err());
}

#[test]
fn remove_empty_role() {
    use many_identity::testing::identity;

    let owner = identity(0);
    let mut account = Account::create(
        &owner,
        CreateArgs {
            description: None,
            roles: None,
            features: Default::default(),
        },
    );
    assert!(!account.has_role(&identity(1), Role::CanMultisigSubmit));
    account.add_role(&identity(1), Role::CanMultisigSubmit);
    assert!(account.has_role(&identity(1), Role::CanMultisigSubmit));

    account.remove_role(&identity(1), Role::CanMultisigSubmit);
    assert!(!account.roles.contains_key(&identity(1)));
}
