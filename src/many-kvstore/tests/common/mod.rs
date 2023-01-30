use many_error::{ManyError, Reason};
use many_identity::testing::identity;
use many_identity::{Address, Identity};
use many_identity_dsa::ecdsa::generate_random_ecdsa_identity;
use many_kvstore::module::KvStoreModuleImpl;
use many_modules::abci_backend::{AbciBlock, ManyAbciModuleBackend};
use many_modules::account;
use many_modules::account::features::FeatureInfo;
use many_modules::account::{AccountModuleBackend, Role};
use many_modules::kvstore::{
    DisableArgs, DisableReturn, GetArgs, GetReturns, KvStoreCommandsModuleBackend,
    KvStoreModuleBackend, PutArgs, QueryArgs, QueryReturns,
};
use once_cell::sync::Lazy;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::BTreeMap;
use std::str::FromStr;

pub static MFX_SYMBOL: Lazy<Address> = Lazy::new(|| {
    Address::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap()
});

pub fn assert_many_err<I: std::fmt::Debug + PartialEq>(r: Result<I, ManyError>, err: ManyError) {
    assert_eq!(r, Err(err));
}

pub struct Setup {
    pub module_impl: KvStoreModuleImpl,
    pub id: Address,
    time: Option<u64>,
}

impl Default for Setup {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Setup {
    pub fn new(blockchain: bool) -> Self {
        let id = generate_random_ecdsa_identity();
        let content = std::fs::read_to_string("../../staging/kvstore_state.json5")
            .or_else(|_| std::fs::read_to_string("staging/kvstore_state.json5"))
            .unwrap();
        let state = json5::from_str(&content).unwrap();
        Self {
            module_impl: KvStoreModuleImpl::new(state, tempfile::tempdir().unwrap(), blockchain)
                .unwrap(),
            id: id.address(),
            time: Some(1_000_000),
        }
    }

    /// Execute a block begin+inner_f+end+commit.
    /// See https://docs.tendermint.com/master/spec/abci/abci.html#block-execution
    pub fn block<R>(&mut self, inner_f: impl FnOnce(&mut Self) -> R) -> (u64, R) {
        if let Some(t) = self.time {
            self.time = Some(t + 1);
        }

        self.module_impl.init().expect("Could not init block");

        self.module_impl
            .begin_block(AbciBlock { time: self.time })
            .expect("Could not begin block");

        let r = inner_f(self);

        self.module_impl.end_block().expect("Could not end block");
        self.module_impl.commit().expect("Could not commit block");

        let info = ManyAbciModuleBackend::info(&self.module_impl).expect("Could not get info.");

        (info.height, r)
    }

    pub fn put(
        &mut self,
        sender: &Address,
        key: Vec<u8>,
        value: Vec<u8>,
        alt_owner: Option<Address>,
    ) -> Result<(), ManyError> {
        self.module_impl.put(
            sender,
            PutArgs {
                key: key.into(),
                value: value.into(),
                alternative_owner: alt_owner,
            },
        )?;
        Ok(())
    }

    pub fn get(&self, sender: &Address, key: Vec<u8>) -> Result<GetReturns, ManyError> {
        self.module_impl.get(sender, GetArgs { key: key.into() })
    }

    pub fn disable(
        &mut self,
        sender: &Address,
        key: Vec<u8>,
        alt_owner: Option<Address>,
        reason: Option<Reason<u64>>,
    ) -> Result<DisableReturn, ManyError> {
        KvStoreCommandsModuleBackend::disable(
            &mut self.module_impl,
            sender,
            DisableArgs {
                key: key.into(),
                alternative_owner: alt_owner,
                reason,
            },
        )
    }

    pub fn query(&self, sender: &Address, key: Vec<u8>) -> Result<QueryReturns, ManyError> {
        self.module_impl
            .query(sender, QueryArgs { key: key.into() })
    }
}

pub fn setup() -> Setup {
    Setup::default()
}

#[derive(Clone)]
#[non_exhaustive]
pub enum AccountType {
    KvStore,
}

fn create_account_args(account_type: AccountType) -> account::CreateArgs {
    let (roles, features) = match account_type {
        AccountType::KvStore => {
            let roles = Some(BTreeMap::from_iter([
                (identity(2), [Role::CanKvStorePut].into()),
                (identity(3), [Role::CanKvStoreDisable].into()),
                // 4 is used in tests, so we skip a few.
                (identity(0x1000), [Role::CanKvStoreTransfer].into()),
            ]));
            let features = account::features::FeatureSet::from_iter([
                account::features::kvstore::AccountKvStore.as_feature(),
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

pub struct SetupWithArgs {
    inner: RefCell<Setup>,
    pub args: account::CreateArgs,
}

impl SetupWithArgs {
    pub fn put(
        &mut self,
        sender: &Address,
        key: Vec<u8>,
        value: Vec<u8>,
        alt_owner: Option<Address>,
    ) -> Result<(), ManyError> {
        self.inner.borrow_mut().put(sender, key, value, alt_owner)
    }

    pub fn get(&self, sender: &Address, key: Vec<u8>) -> Result<GetReturns, ManyError> {
        self.inner.borrow().get(sender, key)
    }

    pub fn disable(
        &mut self,
        sender: &Address,
        key: Vec<u8>,
        alt_owner: Option<Address>,
        reason: Option<Reason<u64>>,
    ) -> Result<DisableReturn, ManyError> {
        self.inner
            .borrow_mut()
            .disable(sender, key, alt_owner, reason)
    }

    pub fn query(&self, sender: &Address, key: Vec<u8>) -> Result<QueryReturns, ManyError> {
        self.inner.borrow().query(sender, key)
    }

    pub fn id(&self) -> Address {
        self.inner.borrow().id
    }

    pub fn module_impl(&self) -> Ref<'_, KvStoreModuleImpl> {
        Ref::map(self.inner.borrow(), |i| &i.module_impl)
    }

    pub fn module_impl_mut(&self) -> RefMut<'_, KvStoreModuleImpl> {
        RefMut::map(self.inner.borrow_mut(), |i| &mut i.module_impl)
    }
}

pub fn setup_with_args(account_type: AccountType) -> SetupWithArgs {
    let setup = Setup::default();
    let args = create_account_args(account_type);

    SetupWithArgs {
        inner: RefCell::new(setup),
        args,
    }
}

pub struct SetupWithAccount {
    pub inner: RefCell<Setup>,
    pub account_id: Address,
}

impl SetupWithAccount {
    pub fn put(
        &mut self,
        sender: &Address,
        key: Vec<u8>,
        value: Vec<u8>,
        alt_owner: Option<Address>,
    ) -> Result<(), ManyError> {
        self.inner.borrow_mut().put(sender, key, value, alt_owner)
    }

    pub fn get(&self, sender: &Address, key: Vec<u8>) -> Result<GetReturns, ManyError> {
        self.inner.borrow().get(sender, key)
    }

    pub fn disable(
        &mut self,
        sender: &Address,
        key: Vec<u8>,
        alt_owner: Option<Address>,
        reason: Option<Reason<u64>>,
    ) -> Result<DisableReturn, ManyError> {
        self.inner
            .borrow_mut()
            .disable(sender, key, alt_owner, reason)
    }

    pub fn query(&self, sender: &Address, key: Vec<u8>) -> Result<QueryReturns, ManyError> {
        self.inner.borrow().query(sender, key)
    }

    pub fn id(&self) -> Address {
        self.inner.borrow().id
    }

    pub fn module_impl(&self) -> Ref<'_, KvStoreModuleImpl> {
        Ref::map(self.inner.borrow(), |s| &s.module_impl)
    }

    pub fn module_impl_mut(&self) -> RefMut<'_, KvStoreModuleImpl> {
        RefMut::map(self.inner.borrow_mut(), |i| &mut i.module_impl)
    }
}

pub fn setup_with_account(account_type: AccountType) -> SetupWithAccount {
    let setup = setup_with_args(account_type);
    let id = setup.id();
    let account = setup
        .inner
        .borrow_mut()
        .module_impl
        .create(&id, setup.args)
        .unwrap();
    SetupWithAccount {
        inner: setup.inner,
        account_id: account.id,
    }
}
