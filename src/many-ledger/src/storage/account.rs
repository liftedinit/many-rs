use crate::error;
use crate::migration::tokens::TOKEN_MIGRATION;
use crate::module::account::{validate_account, verify_account_role};
use crate::storage::multisig::{
    MULTISIG_DEFAULT_EXECUTE_AUTOMATICALLY, MULTISIG_DEFAULT_TIMEOUT_IN_SECS,
    MULTISIG_MAXIMUM_TIMEOUT_IN_SECS,
};
use crate::storage::{LedgerStorage, IDENTITY_ROOT};
use many_error::ManyError;
use many_identity::Address;
use many_modules::account::features::{FeatureId, FeatureInfo, FeatureSet};
use many_modules::account::Role;
use many_modules::{account, events};
use many_types::Either;
use merk::Op;
use std::collections::{BTreeMap, BTreeSet};

pub const ACCOUNT_IDENTITY_ROOT: &str = "/config/account_identity";
pub const ACCOUNT_SUBRESOURCE_ID_ROOT: &str = "/config/account_id";

/// Internal representation of Account metadata
#[derive(Clone, Debug)]
pub struct AccountMeta {
    pub id: Option<Address>,
    pub subresource_id: Option<u32>,
    pub description: Option<String>,
    pub roles: BTreeMap<Address, BTreeSet<Role>>,
    pub features: FeatureSet,
}

pub(super) fn key_for_account(id: &Address) -> Vec<u8> {
    format!("/accounts/{id}").into_bytes()
}

pub fn verify_acl(
    storage: &LedgerStorage,
    sender: &Address,
    addr: &Address,
    roles: impl IntoIterator<Item = Role>,
    feature_id: FeatureId,
) -> Result<(), ManyError> {
    if addr != sender {
        if let Some(account) = storage.get_account(addr)? {
            verify_account_role(&account, sender, feature_id, roles)?;
        } else {
            return Err(error::unauthorized());
        }
    }
    Ok(())
}

impl LedgerStorage {
    /// Create the given accounts in the storage from the Account metadata
    pub fn with_account(
        mut self,
        identity: Option<Address>,
        accounts: Option<Vec<AccountMeta>>,
    ) -> Result<Self, ManyError> {
        if self.migrations.is_active(&TOKEN_MIGRATION) {
            let identity = identity.unwrap_or(self.get_identity(IDENTITY_ROOT)?);
            self.persistent_store
                .apply(&[(
                    ACCOUNT_IDENTITY_ROOT.as_bytes().to_vec(),
                    Op::Put(identity.to_vec()),
                )])
                .map_err(error::storage_apply_failed)?;
        }

        if let Some(accounts) = accounts {
            for account in accounts {
                let id = self._add_account(
                    account::Account {
                        description: account.description.clone(),
                        roles: account.roles,
                        features: account.features,
                        disabled: None,
                    },
                    false,
                )?;

                if account.subresource_id.is_some()
                    && id.subresource_id().is_some()
                    && id.subresource_id() != account.subresource_id
                {
                    return Err(error::unexpected_subresource_id(
                        id.subresource_id().unwrap().to_string(),
                        account.subresource_id.unwrap().to_string(),
                    ));
                }
                if let Some(self_id) = account.id {
                    if id != self_id {
                        return Err(error::unexpected_account_id(id, self_id));
                    }
                }
            }
        }
        Ok(self)
    }

    pub(crate) fn _add_account(
        &mut self,
        mut account: account::Account,
        add_event: bool,
    ) -> Result<Address, ManyError> {
        let id = self.get_next_subresource(ACCOUNT_IDENTITY_ROOT)?;

        // The account MUST own itself.
        account.add_role(&id, account::Role::Owner);

        // Set the multisig threshold properly.
        if let Ok(mut multisig) = account
            .features
            .get::<account::features::multisig::MultisigAccountFeature>()
        {
            multisig.arg.threshold = Some(
                multisig.arg.threshold.unwrap_or(
                    account
                        .roles
                        .iter()
                        .filter(|(_, roles)| {
                            roles.contains(&account::Role::Owner)
                                || roles.contains(&account::Role::CanMultisigApprove)
                                || roles.contains(&account::Role::CanMultisigSubmit)
                        })
                        .count() as u64
                        - 1u64, // We need to subtract one because the account owns itself.
                                // The account can approve but should not be included in the threshold.
                ),
            );
            multisig.arg.timeout_in_secs = Some(
                multisig
                    .arg
                    .timeout_in_secs
                    .map_or(MULTISIG_DEFAULT_TIMEOUT_IN_SECS, |v| {
                        MULTISIG_MAXIMUM_TIMEOUT_IN_SECS.min(v)
                    }),
            );
            multisig.arg.execute_automatically = Some(
                multisig
                    .arg
                    .execute_automatically
                    .unwrap_or(MULTISIG_DEFAULT_EXECUTE_AUTOMATICALLY),
            );

            account.features.insert(multisig.as_feature());
        }

        if add_event {
            self.log_event(events::EventInfo::AccountCreate {
                account: id,
                description: account.clone().description,
                roles: account.clone().roles,
                features: account.clone().features,
            })?;
        }

        self.commit_account(&id, account)?;
        Ok(id)
    }

    pub fn add_account(&mut self, account: account::Account) -> Result<Address, ManyError> {
        let id = self._add_account(account, true)?;
        Ok(id)
    }

    pub fn disable_account(&mut self, id: &Address) -> Result<(), ManyError> {
        let mut account = self
            .get_account_even_disabled(id)?
            .ok_or_else(|| account::errors::unknown_account(*id))?;

        if account.disabled.is_none() || account.disabled == Some(Either::Left(false)) {
            account.disabled = Some(Either::Left(true));
            self.commit_account(id, account)?;
            self.log_event(events::EventInfo::AccountDisable { account: *id })?;

            self.maybe_commit()?;

            Ok(())
        } else {
            Err(account::errors::unknown_account(*id))
        }
    }

    pub fn set_description(
        &mut self,
        mut account: account::Account,
        args: account::SetDescriptionArgs,
    ) -> Result<(), ManyError> {
        account.set_description(Some(args.clone().description));
        self.log_event(events::EventInfo::AccountSetDescription {
            account: args.account,
            description: args.description,
        })?;
        self.commit_account(&args.account, account)?;
        Ok(())
    }

    pub fn add_roles(
        &mut self,
        mut account: account::Account,
        args: account::AddRolesArgs,
    ) -> Result<(), ManyError> {
        for (id, roles) in &args.roles {
            for r in roles {
                account.add_role(id, *r);
            }
        }

        self.log_event(events::EventInfo::AccountAddRoles {
            account: args.account,
            roles: args.clone().roles,
        })?;
        self.commit_account(&args.account, account)?;
        Ok(())
    }

    pub fn remove_roles(
        &mut self,
        mut account: account::Account,
        args: account::RemoveRolesArgs,
    ) -> Result<(), ManyError> {
        // We should not be able to remove the Owner role from the account itself
        if args.roles.contains_key(&args.account)
            && args
                .roles
                .get(&args.account)
                .unwrap()
                .contains(&account::Role::Owner)
        {
            return Err(account::errors::account_must_own_itself());
        }

        for (id, roles) in &args.roles {
            for r in roles {
                account.remove_role(id, *r);
            }
        }

        self.log_event(events::EventInfo::AccountRemoveRoles {
            account: args.account,
            roles: args.clone().roles,
        })?;
        self.commit_account(&args.account, account)?;
        Ok(())
    }

    pub fn add_features(
        &mut self,
        mut account: account::Account,
        args: account::AddFeaturesArgs,
    ) -> Result<(), ManyError> {
        for new_f in args.features.iter() {
            if account.features.insert(new_f.clone()) {
                return Err(ManyError::unknown("Feature already part of the account."));
            }
        }
        if let Some(ref r) = args.roles {
            for (id, new_r) in r {
                for role in new_r {
                    account.roles.entry(*id).or_default().insert(*role);
                }
            }
        }

        validate_account(&account)?;

        self.log_event(events::EventInfo::AccountAddFeatures {
            account: args.account,
            roles: args.clone().roles.unwrap_or_default(), // TODO: Verify this
            features: args.clone().features,
        })?;
        self.commit_account(&args.account, account)?;
        Ok(())
    }

    pub fn get_account(&self, id: &Address) -> Result<Option<account::Account>, ManyError> {
        Ok(self.get_account_even_disabled(id)?.and_then(|x| {
            if x.disabled.is_none() || x.disabled == Some(Either::Left(false)) {
                Some(x)
            } else {
                None
            }
        }))
    }

    pub fn get_account_even_disabled(
        &self,
        id: &Address,
    ) -> Result<Option<account::Account>, ManyError> {
        // TODO: Refactor
        Ok(
            if let Some(bytes) = self
                .persistent_store
                .get(&key_for_account(id))
                .unwrap_or_default()
            {
                Some(
                    minicbor::decode::<account::Account>(&bytes)
                        .map_err(ManyError::deserialization_error)?,
                )
            } else {
                None
            },
        )
    }

    pub fn commit_account(
        &mut self,
        id: &Address,
        account: account::Account,
    ) -> Result<(), ManyError> {
        tracing::debug!("commit({:?})", account);

        self.persistent_store
            .apply(&[(
                key_for_account(id),
                Op::Put(minicbor::to_vec(account).map_err(ManyError::serialization_error)?),
            )])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        self.maybe_commit()?;

        Ok(())
    }
}
