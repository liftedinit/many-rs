use crate::server::module::EmptyReturn;
use crate::types::VecOrSingle;
use crate::{Identity, ManyError};
use many_macros::many_module;
use minicbor::{Decode, Encode};
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use mockall::{automock, predicate::*};

pub mod errors;
pub mod features;

pub type SetDescriptionReturn = EmptyReturn;
pub type AddRolesReturn = EmptyReturn;
pub type RemoveRolesReturn = EmptyReturn;
pub type DeleteReturn = EmptyReturn;
pub type AddFeaturesReturn = EmptyReturn;

/// A generic Account type. This is useful as utility for managing accounts in your backend.
#[derive(Clone, Debug)]
pub struct Account {
    description: Option<String>,
    roles: BTreeMap<Identity, BTreeSet<String>>,
    features: features::FeatureSet,
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
        roles
            .entry(*sender)
            .or_default()
            .insert("owner".to_string());
        Self {
            description,
            roles,
            features,
        }
    }

    pub fn description(&self) -> Option<&String> {
        self.description.as_ref()
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

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
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

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct SetDescriptionArgs {
    #[n(0)]
    pub id: Identity,

    #[n(1)]
    pub description: String,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct ListRolesArgs {
    #[n(0)]
    pub account: Identity,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct ListRolesReturn {
    #[n(0)]
    roles: BTreeSet<String>,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
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
    roles: BTreeMap<Identity, BTreeSet<String>>,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct AddRolesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub roles: BTreeMap<Identity, BTreeSet<String>>,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct RemoveRolesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    pub roles: BTreeMap<Identity, BTreeSet<String>>,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct InfoArgs {
    #[n(0)]
    pub account: Identity,
}

#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct InfoReturn {
    #[n(0)]
    description: Option<String>,

    #[n(1)]
    roles: BTreeMap<Identity, BTreeSet<String>>,

    #[n(2)]
    features: features::FeatureSet,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct DeleteArgs {
    #[n(0)]
    pub account: Identity,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq)]
#[cbor(map)]
pub struct AddFeaturesArgs {
    #[n(0)]
    pub account: Identity,

    #[n(1)]
    roles: Option<BTreeMap<Identity, BTreeSet<String>>>,

    #[n(2)]
    pub features: features::FeatureSet,
}

#[many_module(name = AccountModule, id = 9, namespace = account, many_crate = crate)]
#[cfg_attr(test, automock)]
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
    use mockall::predicate;

    use crate::{
        cbor::CborAny,
        server::module::{
            account::features::{FeatureId, TryCreateFeature},
            testutils::call_module_cbor,
        },
        types::identity::tests,
    };

    use super::{
        features::{Feature, FeatureSet},
        *,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn create() {
        let mut features = FeatureSet::default();
        features.insert(Feature::with_id(1));

        let data = CreateArgs {
            description: Some("Foobar".to_string()),
            roles: None,
            features,
        };

        let mut mock = MockAccountModuleBackend::new();
        mock.expect_create()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(CreateReturn {
                id: Identity::anonymous(),
            }));
        let module = super::AccountModule::new(Arc::new(Mutex::new(mock)));

        let create_return: CreateReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "account.create",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(create_return.id, Identity::anonymous());
    }

    #[test]
    fn set_description() {
        let data = SetDescriptionArgs {
            id: Identity::anonymous(),
            description: "Foobar".to_string(),
        };

        let mut mock = MockAccountModuleBackend::new();
        mock.expect_set_description()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(SetDescriptionReturn {}));
        let module = super::AccountModule::new(Arc::new(Mutex::new(mock)));

        let _: SetDescriptionReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "account.setDescription",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn list_roles() {
        let data = ListRolesArgs {
            account: tests::identity(1),
        };
        let mut mock = MockAccountModuleBackend::new();
        mock.expect_list_roles()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(ListRolesReturn {
                roles: BTreeSet::from(["owner".to_string()]),
            }));
        let module = super::AccountModule::new(Arc::new(Mutex::new(mock)));

        let list_roles_return: ListRolesReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "account.listRoles",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            list_roles_return.roles,
            BTreeSet::from(["owner".to_string()])
        );
    }

    #[test]
    fn get_roles() {
        let data = GetRolesArgs {
            account: tests::identity(1),
            identities: VecOrSingle(vec![Identity::anonymous()]),
        };
        let mut mock = MockAccountModuleBackend::new();
        mock.expect_get_roles()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(GetRolesReturn {
                roles: BTreeMap::from([(
                    Identity::anonymous(),
                    BTreeSet::from(["owner".to_string()]),
                )]),
            }));
        let module = super::AccountModule::new(Arc::new(Mutex::new(mock)));

        let get_roles_return: GetRolesReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "account.getRoles",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            get_roles_return.roles,
            BTreeMap::from([(Identity::anonymous(), BTreeSet::from(["owner".to_string()]),)])
        );
    }

    #[test]
    fn add_roles() {
        let data = AddRolesArgs {
            account: tests::identity(1),
            roles: BTreeMap::from([(tests::identity(2), BTreeSet::from(["foobar".to_string()]))]),
        };
        let mut mock = MockAccountModuleBackend::new();
        mock.expect_add_roles()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(AddRolesReturn {}));
        let module = super::AccountModule::new(Arc::new(Mutex::new(mock)));

        let _: AddRolesReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "account.addRoles",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn remove_roles() {
        let data = RemoveRolesArgs {
            account: tests::identity(1),
            roles: BTreeMap::from([(tests::identity(2), BTreeSet::from(["foobar".to_string()]))]),
        };
        let mut mock = MockAccountModuleBackend::new();
        mock.expect_remove_roles()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(RemoveRolesReturn {}));
        let module = super::AccountModule::new(Arc::new(Mutex::new(mock)));

        let _: RemoveRolesReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "account.removeRoles",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn info() {
        let mut features = FeatureSet::default();
        features.insert(Feature::with_id(1));

        let data = InfoArgs {
            account: tests::identity(1),
        };
        let mut mock = MockAccountModuleBackend::new();
        mock.expect_info()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(InfoReturn {
                description: Some("Foobar".to_string()),
                roles: BTreeMap::from([(tests::identity(2), BTreeSet::from(["foo".to_string()]))]),
                features: features.clone(),
            }));
        let module = super::AccountModule::new(Arc::new(Mutex::new(mock)));

        let info_return: InfoReturn = minicbor::decode(
            &call_module_cbor(1, &module, "account.info", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();

        assert_eq!(info_return.description, Some("Foobar".to_string()));
        assert_eq!(
            info_return.roles,
            BTreeMap::from([(tests::identity(2), BTreeSet::from(["foo".to_string()]))])
        );
        assert_eq!(info_return.features, features);
    }

    #[test]
    fn delete() {
        let data = DeleteArgs {
            account: tests::identity(1),
        };
        let mut mock = MockAccountModuleBackend::new();
        mock.expect_delete()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(DeleteReturn {}));
        let module = super::AccountModule::new(Arc::new(Mutex::new(mock)));

        let _: DeleteReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "account.delete",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn add_features() {
        let mut features = FeatureSet::default();
        features.insert(Feature::with_id(1));

        let data = AddFeaturesArgs {
            account: tests::identity(1),
            roles: Some(BTreeMap::from([(
                tests::identity(2),
                BTreeSet::from(["foobar".to_string()]),
            )])),
            features: features.clone(),
        };
        let mut mock = MockAccountModuleBackend::new();
        mock.expect_add_features()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(AddFeaturesReturn {}));
        let module = super::AccountModule::new(Arc::new(Mutex::new(mock)));

        let _: AddFeaturesReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "account.addFeatures",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn account() {
        let id = tests::identity(1);
        let mut features = FeatureSet::default();
        features.insert(Feature::with_id(1));

        let args = CreateArgs {
            description: Some("Foobar".to_string()),
            roles: Some(BTreeMap::from([(
                tests::identity(2),
                BTreeSet::from(["foobar".to_string()]),
            )])),
            features,
        };
        let account = Account::create(&id, args);

        assert_eq!(account.description(), Some(&"Foobar".to_string()));
        assert_eq!(
            account.roles(),
            &BTreeMap::from([
                (id, BTreeSet::from(["owner".to_string()])),
                (tests::identity(2), BTreeSet::from(["foobar".to_string()]),)
            ])
        );
        assert!(account.has_role(&id, "owner"));

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
        assert!(account.feature::<SomeFeature>().is_none());
    }
}
