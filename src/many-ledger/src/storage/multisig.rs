use crate::error;
use crate::migration::block_9400::Block9400Tx;
use crate::migration::memo::MEMO_MIGRATION;
use crate::module::account::validate_account;
use crate::storage::event::EVENT_ID_KEY_SIZE_IN_BYTES;
use crate::storage::LedgerStorage;
use many_error::ManyError;
use many_identity::Address;
use many_modules::account::features::FeatureInfo;
use many_modules::{account, events, EmptyReturn};
use many_protocol::ResponseMessage;
use many_types::{SortOrder, Timestamp};
use merk::Op;
use std::collections::BTreeMap;
use tracing::debug;

pub(crate) const MULTISIG_TRANSACTIONS_ROOT: &[u8] = b"/multisig/";

/// Returns the storage key for a multisig pending transaction.
pub(super) fn key_for_multisig_transaction(token: &[u8]) -> Vec<u8> {
    let token = if token.len() > EVENT_ID_KEY_SIZE_IN_BYTES {
        &token[0..EVENT_ID_KEY_SIZE_IN_BYTES]
    } else {
        token
    };

    let mut exp_token = [0u8; EVENT_ID_KEY_SIZE_IN_BYTES];
    exp_token[(EVENT_ID_KEY_SIZE_IN_BYTES - token.len())..].copy_from_slice(token);

    vec![MULTISIG_TRANSACTIONS_ROOT, &exp_token[..]]
        .concat()
        .to_vec()
}

fn _execute_multisig_tx(
    ledger: &mut LedgerStorage,
    _tx_id: &[u8],
    storage: &MultisigTransactionStorage,
) -> Result<Vec<u8>, ManyError> {
    let sender = &storage.account;
    match &storage.info.transaction {
        events::AccountMultisigTransaction::Send(many_modules::ledger::SendArgs {
            from,
            to,
            symbol,
            amount,
            memo,
        }) => {
            // Use the `from` field to resolve the account sending the funds
            let from = from.ok_or_else(ManyError::invalid_from_identity)?;

            // The account executing the transaction should have the rights to send the funds
            let account = ledger
                .get_account(&from)?
                .ok_or_else(|| account::errors::unknown_account(from))?;
            account.needs_role(
                sender,
                [account::Role::CanLedgerTransact, account::Role::Owner],
            )?;

            ledger.send(&from, to, symbol, amount.clone(), memo.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountCreate(args) => {
            let account = account::Account::create(sender, args.clone());
            validate_account(&account)?;

            let id = ledger.add_account(account)?;
            minicbor::to_vec(account::CreateReturn { id })
        }

        events::AccountMultisigTransaction::AccountDisable(args) => {
            let account = ledger
                .get_account(&args.account)?
                .ok_or_else(|| account::errors::unknown_account(args.account))?;

            account.needs_role(sender, [account::Role::Owner])?;
            ledger.disable_account(&args.account)?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountSetDescription(args) => {
            let account = ledger
                .get_account(&args.account)?
                .ok_or_else(|| account::errors::unknown_account(args.account))?;

            account.needs_role(sender, [account::Role::Owner])?;
            ledger.set_description(account, args.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountAddRoles(args) => {
            let account = ledger
                .get_account(&args.account)?
                .ok_or_else(|| account::errors::unknown_account(args.account))?;
            account.needs_role(sender, [account::Role::Owner])?;
            ledger.add_roles(account, args.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountRemoveRoles(args) => {
            let account = ledger
                .get_account(&args.account)?
                .ok_or_else(|| account::errors::unknown_account(args.account))?;
            account.needs_role(sender, [account::Role::Owner])?;
            ledger.remove_roles(account, args.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountAddFeatures(args) => {
            let account = ledger
                .get_account(&args.account)?
                .ok_or_else(|| account::errors::unknown_account(args.account))?;

            account.needs_role(sender, [account::Role::Owner])?;
            ledger.add_features(account, args.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigSubmit(arg) => {
            let token = ledger.create_multisig_transaction(sender, arg.clone())?;
            minicbor::to_vec(account::features::multisig::SubmitTransactionReturn {
                token: token.into(),
            })
        }

        events::AccountMultisigTransaction::AccountMultisigSetDefaults(arg) => {
            ledger.set_multisig_defaults(sender, arg.clone())?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigApprove(arg) => {
            ledger.approve_multisig(sender, &arg.token)?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigRevoke(arg) => {
            ledger.revoke_multisig(sender, &arg.token)?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigExecute(arg) => {
            ledger.execute_multisig(sender, &arg.token)?;
            minicbor::to_vec(EmptyReturn)
        }

        events::AccountMultisigTransaction::AccountMultisigWithdraw(arg) => {
            ledger.withdraw_multisig(sender, &arg.token)?;
            minicbor::to_vec(EmptyReturn)
        }

        _ => return Err(account::features::multisig::errors::transaction_type_unsupported()),
    }
    .map_err(ManyError::serialization_error)
}

#[derive(minicbor::Encode, minicbor::Decode, Debug)]
#[cbor(map)]
pub struct MultisigTransactionStorage {
    #[n(0)]
    pub account: Address,

    #[n(1)]
    pub info: account::features::multisig::InfoReturn,

    /// TODO: update this to use timestamp, but this will be a breaking change
    ///       and will require a migration.
    #[n(2)]
    pub creation: std::time::SystemTime,

    #[n(3)]
    pub disabled: bool,
}

impl MultisigTransactionStorage {
    pub fn disable(&mut self, state: account::features::multisig::MultisigTransactionState) {
        self.disabled = true;
        self.info.state = state;
    }

    pub fn should_execute(&self) -> bool {
        self.info.approvers.values().filter(|i| i.approved).count() >= self.info.threshold as usize
    }
}

pub const MULTISIG_DEFAULT_THRESHOLD: u64 = 1;
pub const MULTISIG_DEFAULT_TIMEOUT_IN_SECS: u64 = 60 * 60 * 24; // A day.
pub const MULTISIG_DEFAULT_EXECUTE_AUTOMATICALLY: bool = false;
pub const MULTISIG_MAXIMUM_TIMEOUT_IN_SECS: u64 = 185 * 60 * 60 * 24; // ~6 months.

impl LedgerStorage {
    pub fn check_timed_out_multisig_transactions(&mut self) -> Result<(), ManyError> {
        let it = self.iter_multisig(SortOrder::Descending);
        let mut batch = vec![];

        for item in it {
            let (k, v) = item.map_err(ManyError::unknown)?;

            let mut storage: MultisigTransactionStorage =
                minicbor::decode(v.as_slice()).map_err(ManyError::deserialization_error)?;
            let now = self.now();

            if now >= storage.info.timeout {
                if !storage.disabled {
                    storage.disable(account::features::multisig::MultisigTransactionState::Expired);

                    if let Ok(v) = minicbor::to_vec(storage) {
                        batch.push((k.to_vec(), Op::Put(v)));
                    }
                }
            } else if let Ok(d) = now.as_system_time()?.duration_since(storage.creation) {
                // Since the DB is ordered by event ID (keys), at this point we don't need
                // to continue since we know that the rest is all timed out anyway.
                if d.as_secs() > MULTISIG_MAXIMUM_TIMEOUT_IN_SECS {
                    break;
                }
            }
        }

        if !batch.is_empty() {
            // Reverse the batch so keys are in sorted order.
            batch.reverse();
            self.persistent_store
                .apply(&batch)
                .map_err(error::storage_apply_failed)?;
        }

        self.maybe_commit()?;

        Ok(())
    }

    pub fn set_multisig_defaults(
        &mut self,
        sender: &Address,
        args: account::features::multisig::SetDefaultsArgs,
    ) -> Result<(), ManyError> {
        // Verify the sender has the rights to the account.
        let mut account = self
            .get_account(&args.account)?
            .ok_or_else(|| account::errors::unknown_account(args.account.to_string()))?;

        account.needs_role(sender, [account::Role::Owner])?;

        // Set the multisig threshold properly.
        if let Ok(mut multisig) = account
            .features
            .get::<account::features::multisig::MultisigAccountFeature>()
        {
            if let Some(threshold) = args.threshold {
                multisig.arg.threshold = Some(threshold);
            }
            let timeout_in_secs = args
                .timeout_in_secs
                .map(|t| t.min(MULTISIG_MAXIMUM_TIMEOUT_IN_SECS));
            if let Some(timeout_in_secs) = timeout_in_secs {
                multisig.arg.timeout_in_secs = Some(timeout_in_secs);
            }
            if let Some(execute_automatically) = args.execute_automatically {
                multisig.arg.execute_automatically = Some(execute_automatically);
            }

            account.features.insert(multisig.as_feature());
            self.log_event(events::EventInfo::AccountMultisigSetDefaults {
                submitter: *sender,
                account: args.account,
                threshold: args.threshold,
                timeout_in_secs,
                execute_automatically: args.execute_automatically,
            })?;
            self.commit_account(&args.account, account)?;
        }
        Ok(())
    }

    pub fn commit_multisig_transaction(
        &mut self,
        tx_id: &[u8],
        tx: &MultisigTransactionStorage,
    ) -> Result<(), ManyError> {
        debug!("{:?}", tx);
        self.persistent_store
            .apply(&[(
                key_for_multisig_transaction(tx_id),
                Op::Put(minicbor::to_vec(tx).map_err(ManyError::serialization_error)?),
            )])
            .map_err(error::storage_apply_failed)?;

        self.maybe_commit()?;
        Ok(())
    }

    pub fn create_multisig_transaction(
        &mut self,
        sender: &Address,
        arg: account::features::multisig::SubmitTransactionArgs,
    ) -> Result<Vec<u8>, ManyError> {
        let event_id = self.new_event_id();
        let account_id = arg.account;

        let account = self
            .get_account(&account_id)?
            .ok_or_else(|| account::errors::unknown_account(account_id))?;

        let is_owner = account.has_role(sender, "owner");
        account.needs_role(
            sender,
            [account::Role::CanMultisigSubmit, account::Role::Owner],
        )?;

        let multisig_f = account
            .features
            .get::<account::features::multisig::MultisigAccountFeature>()?;

        let threshold = match arg.threshold {
            Some(t) if is_owner => t,
            Some(_) => return Err(account::errors::user_needs_role("owner")),
            _ => multisig_f
                .arg
                .threshold
                .unwrap_or(MULTISIG_DEFAULT_THRESHOLD),
        };
        let timeout_in_secs = match arg.timeout_in_secs {
            Some(t) if is_owner => t,
            Some(_) => return Err(account::errors::user_needs_role("owner")),
            _ => multisig_f
                .arg
                .timeout_in_secs
                .unwrap_or(MULTISIG_DEFAULT_TIMEOUT_IN_SECS),
        }
        .min(MULTISIG_MAXIMUM_TIMEOUT_IN_SECS);
        let execute_automatically = match arg.execute_automatically {
            Some(e) if is_owner => e,
            Some(_) => return Err(account::errors::user_needs_role("owner")),
            _ => multisig_f
                .arg
                .execute_automatically
                .unwrap_or(MULTISIG_DEFAULT_EXECUTE_AUTOMATICALLY),
        };
        let time = self.now();

        // Set the approvers list to include the sender as true.
        let approvers = BTreeMap::from_iter([(
            *sender,
            account::features::multisig::ApproverInfo { approved: true },
        )]);

        let timeout = Timestamp::from_system_time(
            time.as_system_time()?
                .checked_add(std::time::Duration::from_secs(timeout_in_secs))
                .ok_or_else(|| ManyError::unknown("Invalid time.".to_string()))?,
        )?;

        // If the migration hasn't been applied yet, use the old fields and skip
        // the new memo field. If it has, ignore the old fields and only use the
        // new field.
        let (memo_, data_, memo) = if self.migrations.is_active(&MEMO_MIGRATION) {
            (None, None, arg.memo.clone())
        } else {
            (arg.memo_, arg.data_, None)
        };

        let storage = MultisigTransactionStorage {
            account: account_id,
            info: account::features::multisig::InfoReturn {
                memo_: memo_.clone(),
                memo: memo.clone(),
                transaction: arg.transaction.as_ref().clone(),
                submitter: *sender,
                approvers,
                threshold,
                execute_automatically,
                timeout,
                data_: data_.clone(),
                state: account::features::multisig::MultisigTransactionState::Pending,
            },
            creation: self.now().as_system_time()?,
            disabled: false,
        };

        self.commit_multisig_transaction(event_id.as_ref(), &storage)?;
        self.log_event(events::EventInfo::AccountMultisigSubmit {
            submitter: *sender,
            account: account_id,
            memo_,
            transaction: Box::new(*arg.transaction),
            token: Some(event_id.clone().into()),
            threshold,
            timeout,
            execute_automatically,
            data_,
            memo,
        })?;

        Ok(event_id.into())
    }

    pub fn get_multisig_info(&self, tx_id: &[u8]) -> Result<MultisigTransactionStorage, ManyError> {
        let storage_bytes = self
            .persistent_store
            .get(&key_for_multisig_transaction(tx_id))
            .unwrap_or(None)
            .ok_or_else(account::features::multisig::errors::transaction_cannot_be_found)?;
        minicbor::decode::<MultisigTransactionStorage>(&storage_bytes)
            .map_err(ManyError::deserialization_error)
    }

    pub fn approve_multisig(&mut self, sender: &Address, tx_id: &[u8]) -> Result<bool, ManyError> {
        let mut storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(account::features::multisig::errors::transaction_expired_or_withdrawn());
        }

        let account = self
            .get_account(&storage.account)?
            .ok_or_else(|| account::errors::unknown_account(storage.account.to_string()))?;

        // Validate the right.
        if !account.has_role(sender, account::Role::CanMultisigApprove)
            && !account.has_role(sender, account::Role::CanMultisigSubmit)
            && !account.has_role(sender, account::Role::Owner)
        {
            return Err(account::features::multisig::errors::user_cannot_approve_transaction());
        }

        // Update the entry.
        storage.info.approvers.entry(*sender).or_default().approved = true;

        self.commit_multisig_transaction(tx_id, &storage)?;
        self.log_event(events::EventInfo::AccountMultisigApprove {
            account: storage.account,
            token: tx_id.to_vec().into(),
            approver: *sender,
        })?;

        // If the transaction executes automatically, calculate number of approvers.
        if storage.info.execute_automatically && storage.should_execute() {
            let response = self.execute_multisig_transaction_internal(tx_id, &storage, true)?;
            self.log_event(events::EventInfo::AccountMultisigExecute {
                account: storage.account,
                token: tx_id.to_vec().into(),
                executer: None,
                response,
            })?;
            return Ok(true);
        }

        Ok(false)
    }

    pub fn revoke_multisig(&mut self, sender: &Address, tx_id: &[u8]) -> Result<bool, ManyError> {
        let mut storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(account::features::multisig::errors::transaction_expired_or_withdrawn());
        }

        let account = self
            .get_account(&storage.account)?
            .ok_or_else(|| account::errors::unknown_account(storage.account.to_string()))?;

        // We make an exception here for people who already approved.
        if let Some(info) = storage.info.approvers.get_mut(sender) {
            info.approved = false;
        } else if account.has_role(sender, account::Role::CanMultisigSubmit)
            || account.has_role(sender, account::Role::CanMultisigApprove)
            || account.has_role(sender, account::Role::Owner)
        {
            storage.info.approvers.entry(*sender).or_default().approved = false;
        } else {
            return Err(account::features::multisig::errors::user_cannot_approve_transaction());
        }

        self.commit_multisig_transaction(tx_id, &storage)?;
        self.log_event(events::EventInfo::AccountMultisigRevoke {
            account: storage.account,
            token: tx_id.to_vec().into(),
            revoker: *sender,
        })?;
        Ok(false)
    }

    pub fn execute_multisig(
        &mut self,
        sender: &Address,
        tx_id: &[u8],
    ) -> Result<ResponseMessage, ManyError> {
        let storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(account::features::multisig::errors::transaction_expired_or_withdrawn());
        }

        // Verify the sender has the rights to the account.
        let account = self
            .get_account(&storage.account)?
            .ok_or_else(|| account::errors::unknown_account(storage.account.to_string()))?;

        // TODO: Better error message
        if !(account.has_role(sender, account::Role::Owner) || storage.info.submitter == *sender) {
            return Err(account::features::multisig::errors::cannot_execute_transaction());
        }

        if storage.should_execute() {
            let response = self.execute_multisig_transaction_internal(tx_id, &storage, false)?;
            self.log_event(events::EventInfo::AccountMultisigExecute {
                account: storage.account,
                token: tx_id.to_vec().into(),
                executer: Some(*sender),
                response: response.clone(),
            })?;
            Ok(response)
        } else {
            Err(account::features::multisig::errors::cannot_execute_transaction())
        }
    }

    pub fn withdraw_multisig(&mut self, sender: &Address, tx_id: &[u8]) -> Result<(), ManyError> {
        let storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(account::features::multisig::errors::transaction_expired_or_withdrawn());
        }

        // Verify the sender has the rights to the account.
        let account = self
            .get_account(&storage.account)?
            .ok_or_else(|| account::errors::unknown_account(storage.account.to_string()))?;

        if !(account.has_role(sender, "owner") || storage.info.submitter == *sender) {
            return Err(account::features::multisig::errors::cannot_execute_transaction());
        }

        self.disable_multisig_transaction(
            tx_id,
            account::features::multisig::MultisigTransactionState::Withdrawn,
        )?;
        self.log_event(events::EventInfo::AccountMultisigWithdraw {
            account: storage.account,
            token: tx_id.to_vec().into(),
            withdrawer: *sender,
        })?;
        Ok(())
    }

    fn disable_multisig_transaction(
        &mut self,
        tx_id: &[u8],
        state: account::features::multisig::MultisigTransactionState,
    ) -> Result<(), ManyError> {
        let mut storage = self.get_multisig_info(tx_id)?;
        if storage.disabled {
            return Err(account::features::multisig::errors::transaction_expired_or_withdrawn());
        }
        storage.disable(state);

        let v =
            minicbor::to_vec(storage).map_err(|e| ManyError::serialization_error(e.to_string()))?;

        self.persistent_store
            .apply(&[(key_for_multisig_transaction(tx_id), Op::Put(v))])
            .map_err(error::storage_apply_failed)?;

        self.maybe_commit()?;
        Ok(())
    }

    fn execute_multisig_transaction_internal(
        &mut self,
        tx_id: &[u8],
        storage: &MultisigTransactionStorage,
        automatic: bool,
    ) -> Result<ResponseMessage, ManyError> {
        let result = _execute_multisig_tx(self, tx_id, storage);

        self.disable_multisig_transaction(
            tx_id,
            if automatic {
                account::features::multisig::MultisigTransactionState::ExecutedAutomatically
            } else {
                account::features::multisig::MultisigTransactionState::ExecutedManually
            },
        )?;

        let response = ResponseMessage {
            from: storage.account,
            to: None,
            data: result,
            timestamp: Some(self.now()),
            ..Default::default()
        };

        let response = self
            .block_hotfix("Block 9400", || Block9400Tx::new(tx_id, response.clone()))?
            .unwrap_or(response);

        #[cfg(feature = "migration_testing")]
        let response = self
            .block_hotfix("Dummy Hotfix", || {
                crate::migration::dummy_hotfix::DummyHotfix::new(tx_id, response.clone())
            })?
            .unwrap_or(response);

        Ok(response)
    }
}
