use super::KvStoreStorage;
use crate::module::account::validate_account;
use many_error::ManyError;
use many_identity::Address;
use many_modules::{account, events};
use many_types::Either;
use merk::Op;

fn key_for_account(id: &Address) -> Vec<u8> {
    format!("/accounts/{id}").into_bytes()
}

impl KvStoreStorage {
    pub(crate) fn _add_account(
        &mut self,
        mut account: account::Account,
        add_event: bool,
    ) -> Result<Address, ManyError> {
        let id = self.new_subresource_id()?;

        // The account MUST own itself.
        account.add_role(&id, account::Role::Owner);

        if add_event {
            self.log_event(events::EventInfo::AccountCreate {
                account: id,
                description: account.clone().description,
                roles: account.clone().roles,
                features: account.clone().features,
            });
        }

        self.commit_account(&id, account)?;
        Ok(id)
    }

    pub fn add_account(&mut self, account: account::Account) -> Result<Address, ManyError> {
        let id = self._add_account(account, true)?;
        Ok(id)
    }

    pub fn get_account(&self, id: &Address) -> Option<account::Account> {
        self.get_account_even_disabled(id).and_then(|x| {
            if x.disabled.is_none() || x.disabled == Some(Either::Left(false)) {
                Some(x)
            } else {
                None
            }
        })
    }

    pub fn get_account_even_disabled(&self, id: &Address) -> Option<account::Account> {
        self.persistent_store
            .get(&key_for_account(id))
            .unwrap_or_default()
            .as_ref()
            .and_then(|bytes| {
                minicbor::decode::<account::Account>(bytes)
                    .map_err(|e| ManyError::deserialization_error(e.to_string()))
                    .ok()
            })
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
        });
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
        });
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
        });
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
        });
        self.commit_account(&args.account, account)?;
        Ok(())
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
                Op::Put(
                    minicbor::to_vec(account)
                        .map_err(|e| ManyError::serialization_error(e.to_string()))?,
                ),
            )])
            .map_err(|e| ManyError::unknown(e.to_string()))?;

        if !self.blockchain {
            self.persistent_store
                .commit(&[])
                .expect("Could not commit to store.");
        }
        Ok(())
    }

    pub fn disable_account(&mut self, id: &Address) -> Result<(), ManyError> {
        let mut account = self
            .get_account_even_disabled(id)
            .ok_or_else(|| account::errors::unknown_account(*id))?;

        if account.disabled.is_none() || account.disabled == Some(Either::Left(false)) {
            account.disabled = Some(Either::Left(true));
            self.commit_account(id, account)?;
            self.log_event(events::EventInfo::AccountDisable { account: *id });

            if !self.blockchain {
                self.persistent_store
                    .commit(&[])
                    .expect("Could not commit to store.");
            }

            Ok(())
        } else {
            Err(account::errors::unknown_account(*id))
        }
    }
}
