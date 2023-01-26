pub mod cucumber;

use coset::CborSerializable;
use itertools::Itertools;
use many_error::ManyError;
use many_identity::testing::identity;
use many_identity::{Address, Identity};
use many_identity_dsa::ed25519::generate_random_ed25519_identity;
use many_ledger::json::InitialStateJson;
use many_ledger::module::LedgerModuleImpl;
use many_migration::{InnerMigration, MigrationConfig};
use many_modules::abci_backend::{AbciBlock, ManyAbciModuleBackend};
use many_modules::account::features::multisig::{
    AccountMultisigModuleBackend, ExecuteArgs, InfoReturn,
};
use many_modules::account::features::FeatureInfo;
use many_modules::account::AccountModuleBackend;
use many_modules::idstore::{CredentialId, PublicKey};
use many_modules::ledger::extended_info::visual_logo::VisualTokenLogo;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{
    BalanceArgs, LedgerCommandsModuleBackend, LedgerModuleBackend, TokenCreateArgs,
};
use many_modules::{account, events, ledger};
use many_protocol::ResponseMessage;
use many_types::ledger::{
    LedgerTokensAddressMap, Symbol, TokenAmount, TokenInfoSummary, TokenMaybeOwner,
};
use many_types::Memo;
use merk::Merk;
use minicbor::bytes::ByteVec;
use once_cell::sync::Lazy;
use proptest::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

pub fn default_token_create_args(
    owner: Option<TokenMaybeOwner>,
    maximum_supply: Option<TokenAmount>,
) -> TokenCreateArgs {
    let mut logos = VisualTokenLogo::new();
    logos.unicode_front('∑');
    TokenCreateArgs {
        summary: TokenInfoSummary {
            name: "Test Token".to_string(),
            ticker: "TT".to_string(),
            decimals: 9,
        },
        owner,
        initial_distribution: Some(LedgerTokensAddressMap::from([
            (identity(1), TokenAmount::from(123u64)),
            (identity(2), TokenAmount::from(456u64)),
            (identity(3), TokenAmount::from(789u64)),
        ])),
        maximum_supply,
        extended_info: Some(
            TokenExtendedInfo::new()
                .with_memo("Foofoo".try_into().unwrap())
                .unwrap()
                .with_visual_logo(logos)
                .unwrap(),
        ),
        memo: None,
    }
}

pub struct MigrationHarness {
    inner: &'static InnerMigration<merk::Merk, ManyError>,
    block_height: u64,
    enabled: bool,
}

impl MigrationHarness {
    pub fn to_json_str(&self) -> String {
        let maybe_enabled = if !self.enabled {
            r#", "disabled": true"#
        } else {
            ""
        };

        format!(
            r#"{{ "name": "{}", "block_height": {}, "issue": "" {maybe_enabled} }}"#,
            self.inner.name(),
            self.block_height
        )
    }
}

impl From<(u64, &'static InnerMigration<merk::Merk, ManyError>)> for MigrationHarness {
    fn from((block_height, inner): (u64, &'static InnerMigration<Merk, ManyError>)) -> Self {
        MigrationHarness {
            inner,
            block_height,
            enabled: true,
        }
    }
}

impl From<(u64, &'static InnerMigration<merk::Merk, ManyError>, bool)> for MigrationHarness {
    fn from(
        (block_height, inner, enabled): (u64, &'static InnerMigration<Merk, ManyError>, bool),
    ) -> Self {
        MigrationHarness {
            inner,
            block_height,
            enabled,
        }
    }
}

pub static MFX_SYMBOL: Lazy<Address> = Lazy::new(|| {
    Address::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap()
});

pub fn assert_many_err<I: std::fmt::Debug + PartialEq>(r: Result<I, ManyError>, err: ManyError) {
    assert_eq!(r, Err(err));
}

pub fn create_account_args(account_type: AccountType) -> account::CreateArgs {
    let (roles, features) = match account_type {
        AccountType::Multisig => {
            let roles = Some(BTreeMap::from_iter([
                (
                    identity(2),
                    BTreeSet::from_iter([account::Role::CanMultisigApprove]),
                ),
                (
                    identity(3),
                    BTreeSet::from_iter([account::Role::CanMultisigSubmit]),
                ),
            ]));
            let features = account::features::FeatureSet::from_iter([
                account::features::multisig::MultisigAccountFeature::default().as_feature(),
            ]);
            (roles, features)
        }
        AccountType::Ledger => {
            let roles = Some(BTreeMap::from_iter([(
                identity(2),
                BTreeSet::from_iter([account::Role::CanLedgerTransact]),
            )]));
            let features = account::features::FeatureSet::from_iter([
                account::features::ledger::AccountLedger.as_feature(),
            ]);
            (roles, features)
        }
        AccountType::Tokens => {
            let roles = Some(BTreeMap::from_iter([(
                identity(2),
                BTreeSet::from_iter([account::Role::CanTokensCreate]),
            )]));
            let features = account::features::FeatureSet::from_iter([
                account::features::tokens::TokenAccountLedger.as_feature(),
            ]);
            (roles, features)
        }
    };

    account::CreateArgs {
        description: Some("Foobar".to_string()),
        roles,
        features,
    }
}

#[derive(Debug)]
pub struct Setup {
    pub module_impl: LedgerModuleImpl,
    pub id: Address,
    pub cred_id: CredentialId,
    pub public_key: PublicKey,

    time: Option<u64>,
}

impl Default for Setup {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Setup {
    fn _new(
        blockchain: bool,
        migration_config: Option<MigrationConfig>,
        skip_hash_check: bool, // If true, skip the staging file hash check
    ) -> Self {
        let id = generate_random_ed25519_identity();
        let public_key = PublicKey(id.public_key().to_vec().unwrap().into());

        let store_path = tempfile::tempdir().expect("Could not create a temporary dir.");
        tracing::debug!("Store path: {:?}", store_path.path());
        let mut state = InitialStateJson::read("../../staging/ledger_state.json5")
            .or_else(|_| InitialStateJson::read("staging/ledger_state.json5"))
            .expect("Could not read initial state.");

        if skip_hash_check {
            state.hash = None;
        }

        Self {
            module_impl: LedgerModuleImpl::new(state, migration_config, store_path, blockchain)
                .unwrap(),
            id: id.address(),
            cred_id: CredentialId(vec![1; 16].into()),
            public_key,
            time: Some(1_000_000),
        }
    }

    pub fn new(blockchain: bool) -> Self {
        Setup::_new(blockchain, None, false)
    }

    pub fn new_with_migrations(
        blockchain: bool,
        migrations: impl IntoIterator<Item = impl Into<MigrationHarness>>,
        skip_hash_check: bool,
    ) -> Self {
        let migrations = format!(
            r#"{{ "migrations": [{}] }}"#,
            migrations
                .into_iter()
                .map(|x| x.into().to_json_str())
                .join(",")
        );

        Setup::_new(
            blockchain,
            Some(serde_json::from_str(&migrations).unwrap()),
            skip_hash_check,
        )
    }

    pub fn set_balance(&mut self, id: Address, amount: u64, symbol: Symbol) {
        self.module_impl
            .set_balance_only_for_testing(id, amount, symbol)
            .expect("Unable to set balance for testing.");
    }

    pub fn balance(&self, account: Address, symbol: Symbol) -> Result<TokenAmount, ManyError> {
        Ok(self
            .module_impl
            .balance(
                &account,
                BalanceArgs {
                    account: None,
                    symbols: Some(vec![symbol].into()),
                },
            )?
            .balances
            .get(&symbol)
            .cloned()
            .unwrap_or_default())
    }

    pub fn balance_(&self, account: Address) -> TokenAmount {
        self.balance(account, *MFX_SYMBOL).unwrap()
    }

    pub fn send(
        &mut self,
        from: Address,
        to: Address,
        amount: impl Into<TokenAmount>,
        symbol: Symbol,
    ) -> Result<(), ManyError> {
        self.send_as(from, from, to, amount, symbol)
    }

    pub fn send_as(
        &mut self,
        sender: Address,
        from: Address,
        to: Address,
        amount: impl Into<TokenAmount>,
        symbol: Symbol,
    ) -> Result<(), ManyError> {
        self.module_impl.send(
            &sender,
            ledger::SendArgs {
                from: Some(from),
                to,
                amount: amount.into(),
                symbol,
                memo: None,
            },
        )?;
        Ok(())
    }

    pub fn send_(&mut self, from: Address, to: Address, amount: impl Into<TokenAmount>) {
        self.send(from, to, amount, *MFX_SYMBOL)
            .expect("Could not send tokens")
    }

    pub fn create_account_as(
        &mut self,
        id: Address,
        account_type: AccountType,
    ) -> Result<Address, ManyError> {
        let args = create_account_args(account_type);
        AccountModuleBackend::create(&mut self.module_impl, &id, args).map(|x| x.id)
    }

    pub fn create_account(&mut self, account_type: AccountType) -> Result<Address, ManyError> {
        self.create_account_as(self.id, account_type)
    }

    pub fn create_account_(&mut self, account_type: AccountType) -> Address {
        self.create_account(account_type).unwrap()
    }

    pub fn create_account_as_(&mut self, id: Address, account_type: AccountType) -> Address {
        self.create_account_as(id, account_type).unwrap()
    }

    pub fn inc_time(&mut self, amount: u64) {
        self.time = Some(self.time.unwrap_or_default() + amount);
    }

    /// Execute a block begin+inner_f+end+commit.
    /// See https://docs.tendermint.com/master/spec/abci/abci.html#block-execution
    pub fn block<R>(&mut self, inner_f: impl FnOnce(&mut Self) -> R) -> (u64, R) {
        if let Some(t) = self.time {
            self.time = Some(t + 1);
        }

        self.module_impl
            .begin_block(AbciBlock { time: self.time })
            .expect("Could not begin block");

        let r = inner_f(self);

        self.module_impl.end_block().expect("Could not end block");
        self.module_impl.commit().expect("Could not commit block");

        let info = ManyAbciModuleBackend::info(&self.module_impl).expect("Could not get info.");

        (info.height, r)
    }

    pub fn add_roles_as(
        &mut self,
        id: Address,
        account_id: Address,
        roles: BTreeMap<Address, BTreeSet<account::Role>>,
    ) {
        self.module_impl
            .add_roles(
                &id,
                account::AddRolesArgs {
                    account: account_id,
                    roles,
                },
            )
            .unwrap();
    }

    pub fn add_roles(
        &mut self,
        account_id: Address,
        roles: BTreeMap<Address, BTreeSet<account::Role>>,
    ) {
        self.add_roles_as(self.id, account_id, roles);
    }

    /// Create a multisig transaction using the owner ID.
    pub fn create_multisig(
        &mut self,
        account_id: Address,
        event: events::AccountMultisigTransaction,
    ) -> Result<ByteVec, ManyError> {
        self.create_multisig_as(self.id, account_id, event)
    }

    pub fn create_multisig_as(
        &mut self,
        id: Address,
        account_id: Address,
        event: events::AccountMultisigTransaction,
    ) -> Result<ByteVec, ManyError> {
        self.module_impl
            .multisig_submit_transaction(
                &id,
                account::features::multisig::SubmitTransactionArgs {
                    account: account_id,
                    memo: Some(Memo::try_from("Foo".to_string()).unwrap()),
                    transaction: Box::new(event),
                    threshold: None,
                    timeout_in_secs: None,
                    execute_automatically: None,
                    data_: None,
                    memo_: None,
                },
            )
            .map(|x| x.token)
    }

    pub fn create_multisig_(
        &mut self,
        account_id: Address,
        transaction: events::AccountMultisigTransaction,
    ) -> ByteVec {
        self.create_multisig(account_id, transaction).unwrap()
    }

    /// Send some tokens as a multisig transaction.
    pub fn multisig_send(
        &mut self,
        account_id: Address,
        to: Address,
        amount: impl Into<TokenAmount>,
        symbol: Address,
    ) -> Result<ByteVec, ManyError> {
        self.create_multisig(
            account_id,
            events::AccountMultisigTransaction::Send(ledger::SendArgs {
                from: Some(account_id),
                to,
                symbol,
                amount: amount.into(),
                memo: None,
            }),
        )
    }

    pub fn multisig_send_(
        &mut self,
        account_id: Address,
        to: Address,
        amount: impl Into<TokenAmount>,
    ) -> ByteVec {
        self.multisig_send(account_id, to, amount, *MFX_SYMBOL)
            .unwrap()
    }

    /// Approve a multisig transaction.
    pub fn multisig_approve(&mut self, id: Address, token: &ByteVec) -> Result<(), ManyError> {
        let token = token.clone();
        self.module_impl
            .multisig_approve(&id, account::features::multisig::ApproveArgs { token })?;
        Ok(())
    }

    pub fn multisig_approve_(&mut self, id: Address, token: &ByteVec) {
        self.multisig_approve(id, token)
            .expect("Could not approve multisig")
    }

    pub fn multisig_execute_as(
        &mut self,
        id: Address,
        token: &ByteVec,
    ) -> Result<ResponseMessage, ManyError> {
        self.module_impl.multisig_execute(
            &id,
            ExecuteArgs {
                token: token.clone(),
            },
        )
    }

    /// Execute the transaction.
    pub fn multisig_execute(&mut self, token: &ByteVec) -> Result<ResponseMessage, ManyError> {
        self.multisig_execute_as(self.id, token)
    }

    pub fn multisig_execute_as_(&mut self, id: Address, token: &ByteVec) -> ResponseMessage {
        self.multisig_execute_as(id, token).unwrap()
    }

    pub fn multisig_execute_(&mut self, token: &ByteVec) -> ResponseMessage {
        self.multisig_execute(token)
            .expect("Could not execute multisig")
    }

    pub fn assert_multisig_info(&self, token: &ByteVec, assert_f: impl FnOnce(InfoReturn)) {
        let token = token.clone();
        assert_f(
            self.module_impl
                .multisig_info(&self.id, account::features::multisig::InfoArgs { token })
                .expect("Could not find multisig info"),
        );
    }
}

pub fn setup() -> Setup {
    Setup::default()
}

pub struct SetupWithArgs {
    pub module_impl: LedgerModuleImpl,
    pub id: Address,
    pub args: account::CreateArgs,
}

#[derive(Clone)]
#[non_exhaustive]
pub enum AccountType {
    Multisig,
    Ledger,
    Tokens,
}

pub fn setup_with_args(account_type: AccountType) -> SetupWithArgs {
    let setup = Setup::default();
    let args = create_account_args(account_type);

    SetupWithArgs {
        module_impl: setup.module_impl,
        id: setup.id,
        args,
    }
}

pub struct SetupWithAccount {
    pub module_impl: LedgerModuleImpl,
    pub id: Address,
    pub account_id: Address,
}

pub fn setup_with_account(account_type: AccountType) -> SetupWithAccount {
    let SetupWithArgs {
        mut module_impl,
        id,
        args,
    } = setup_with_args(account_type);
    let account = AccountModuleBackend::create(&mut module_impl, &id, args).unwrap();
    SetupWithAccount {
        module_impl,
        id,
        account_id: account.id,
    }
}

#[derive(Debug)]
pub struct SetupWithAccountAndTx {
    pub module_impl: LedgerModuleImpl,
    pub id: Address,
    pub account_id: Address,
    pub tx: events::AccountMultisigTransaction,
}

fn event_from_kind(
    event: events::EventKind,
    module_impl: &mut LedgerModuleImpl,
    id: Address,
    account_id: Address,
    account_type: AccountType,
) -> events::AccountMultisigTransaction {
    let send_tx = events::AccountMultisigTransaction::Send(ledger::SendArgs {
        from: Some(account_id),
        to: identity(3),
        symbol: *MFX_SYMBOL,
        amount: TokenAmount::from(10u16),
        memo: None,
    });

    match event {
        events::EventKind::Send => send_tx,
        events::EventKind::AccountCreate => {
            events::AccountMultisigTransaction::AccountCreate(create_account_args(account_type))
        }
        events::EventKind::AccountDisable => {
            events::AccountMultisigTransaction::AccountDisable(account::DisableArgs {
                account: account_id,
            })
        }
        events::EventKind::AccountSetDescription => {
            events::AccountMultisigTransaction::AccountSetDescription(account::SetDescriptionArgs {
                account: account_id,
                description: "New description".to_string(),
            })
        }
        events::EventKind::AccountAddRoles => {
            events::AccountMultisigTransaction::AccountAddRoles(account::AddRolesArgs {
                account: account_id,
                roles: BTreeMap::from([(
                    identity(100),
                    BTreeSet::from([account::Role::CanMultisigSubmit]),
                )]),
            })
        }
        events::EventKind::AccountRemoveRoles => {
            events::AccountMultisigTransaction::AccountRemoveRoles(account::RemoveRolesArgs {
                account: account_id,
                roles: BTreeMap::from([(
                    identity(3),
                    BTreeSet::from([account::Role::CanMultisigSubmit]),
                )]),
            })
        }
        events::EventKind::AccountAddFeatures => {
            events::AccountMultisigTransaction::AccountAddFeatures(account::AddFeaturesArgs {
                account: account_id,
                roles: Some(BTreeMap::from([(
                    identity(200),
                    BTreeSet::from([account::Role::CanLedgerTransact]),
                )])),
                features: account::features::FeatureSet::from_iter([
                    account::features::ledger::AccountLedger.as_feature(),
                ]),
            })
        }
        events::EventKind::AccountMultisigSubmit => {
            events::AccountMultisigTransaction::AccountMultisigSubmit(
                account::features::multisig::SubmitTransactionArgs {
                    account: account_id,
                    memo: Some(Memo::try_from("A memo".to_string()).unwrap()),
                    transaction: Box::new(send_tx),
                    threshold: None,
                    timeout_in_secs: None,
                    execute_automatically: Some(false),
                    data_: None,
                    memo_: None,
                },
            )
        }
        events::EventKind::AccountMultisigApprove => {
            let token = module_impl
                .multisig_submit_transaction(
                    &id,
                    account::features::multisig::SubmitTransactionArgs {
                        account: account_id,
                        memo: Some(Memo::try_from("A memo".to_string()).unwrap()),
                        transaction: Box::new(send_tx),
                        threshold: None,
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                        data_: None,
                        memo_: None,
                    },
                )
                .unwrap()
                .token;
            events::AccountMultisigTransaction::AccountMultisigApprove(
                account::features::multisig::ApproveArgs { token },
            )
        }
        events::EventKind::AccountMultisigRevoke => {
            let token = module_impl
                .multisig_submit_transaction(
                    &id,
                    account::features::multisig::SubmitTransactionArgs {
                        account: account_id,
                        memo: Some(Memo::try_from("A memo".to_string()).unwrap()),
                        transaction: Box::new(send_tx),
                        threshold: None,
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                        data_: None,
                        memo_: None,
                    },
                )
                .unwrap()
                .token;

            events::AccountMultisigTransaction::AccountMultisigRevoke(
                account::features::multisig::RevokeArgs { token },
            )
        }
        events::EventKind::AccountMultisigExecute => {
            let token = module_impl
                .multisig_submit_transaction(
                    &id,
                    account::features::multisig::SubmitTransactionArgs {
                        account: account_id,
                        memo: Some(Memo::try_from("A memo".to_string()).unwrap()),
                        transaction: Box::new(send_tx),
                        threshold: None,
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                        data_: None,
                        memo_: None,
                    },
                )
                .unwrap()
                .token;
            // Pre-approve the transaction
            for i in [id, identity(2), identity(3)] {
                let _ = module_impl.multisig_approve(
                    &i,
                    account::features::multisig::ApproveArgs {
                        token: token.clone(),
                    },
                );
            }
            events::AccountMultisigTransaction::AccountMultisigExecute(ExecuteArgs { token })
        }
        events::EventKind::AccountMultisigWithdraw => {
            let token = module_impl
                .multisig_submit_transaction(
                    &id,
                    account::features::multisig::SubmitTransactionArgs {
                        account: account_id,
                        memo: Some(Memo::try_from("A memo".to_string()).unwrap()),
                        transaction: Box::new(send_tx),
                        threshold: None,
                        timeout_in_secs: None,
                        execute_automatically: Some(false),
                        data_: None,
                        memo_: None,
                    },
                )
                .unwrap()
                .token;
            events::AccountMultisigTransaction::AccountMultisigWithdraw(
                account::features::multisig::WithdrawArgs { token },
            )
        }
        events::EventKind::AccountMultisigSetDefaults => {
            events::AccountMultisigTransaction::AccountMultisigSetDefaults(
                account::features::multisig::SetDefaultsArgs {
                    account: account_id,
                    threshold: Some(1),
                    timeout_in_secs: Some(500),
                    execute_automatically: Some(true),
                },
            )
        }
        _ => unimplemented!(),
    }
}

prop_compose! {
    pub fn setup_with_account_and_tx(account_type: AccountType)(event in arb_event_kind()) -> SetupWithAccountAndTx {
        let SetupWithAccount {
            mut module_impl,
            id,
            account_id,
        } = setup_with_account(account_type.clone());

        let event = event_from_kind(event, &mut module_impl, id, account_id, account_type.clone());

        SetupWithAccountAndTx {
            module_impl,
            id,
            account_id,
            tx: event,
        }
    }
}

pub fn verify_balance(
    module_impl: &LedgerModuleImpl,
    id: Address,
    symbol: Address,
    amount: TokenAmount,
) {
    let result = module_impl.balance(
        &id,
        BalanceArgs {
            account: Some(id),
            symbols: Some(vec![symbol].into()),
        },
    );
    assert!(result.is_ok());
    let balances = result.unwrap();
    assert_eq!(balances.balances, BTreeMap::from([(symbol, amount)]));
}

fn arb_event_kind() -> impl Strategy<Value = events::EventKind> {
    prop_oneof![
        // Ledger-related
        Just(events::EventKind::Send),
        // Account-related
        Just(events::EventKind::AccountCreate),
        Just(events::EventKind::AccountDisable),
        Just(events::EventKind::AccountSetDescription),
        Just(events::EventKind::AccountAddRoles),
        Just(events::EventKind::AccountRemoveRoles),
        Just(events::EventKind::AccountAddFeatures),
        // Multisig-related
        Just(events::EventKind::AccountMultisigSubmit),
        Just(events::EventKind::AccountMultisigApprove),
        Just(events::EventKind::AccountMultisigRevoke),
        Just(events::EventKind::AccountMultisigExecute),
        Just(events::EventKind::AccountMultisigWithdraw),
        Just(events::EventKind::AccountMultisigSetDefaults),
    ]
}
