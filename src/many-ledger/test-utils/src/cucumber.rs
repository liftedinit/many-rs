use cucumber::Parameter;
use many_error::{ManyError, ManyErrorCode};
use many_identity::testing::identity;
use many_identity::{Address, Identity};
use many_identity_dsa::ecdsa::generate_random_ecdsa_identity;
use many_ledger::error;
use many_ledger::module::LedgerModuleImpl;
use many_modules::account;
use many_modules::account::features::{FeatureInfo, FeatureSet};
use many_modules::account::{
    AccountModuleBackend, AddRolesArgs, CreateArgs, RemoveRolesArgs, Role,
};
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenInfoArgs};
use many_types::cbor::CborNull;
use many_types::ledger::{TokenAmount, TokenInfo, TokenMaybeOwner};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::str::FromStr;

pub trait LedgerWorld {
    fn setup_id(&self) -> Address;
    fn module_impl(&self) -> &LedgerModuleImpl;
    fn module_impl_mut(&mut self) -> &mut LedgerModuleImpl;
    fn error(&self) -> &Option<ManyError>;
}

pub trait AccountWorld {
    fn account(&self) -> Address;
    fn account_mut(&mut self) -> &mut Address;
}

pub trait TokenWorld {
    fn info(&self) -> &TokenInfo;
    fn info_mut(&mut self) -> &mut TokenInfo;
    fn ext_info_mut(&mut self) -> &mut TokenExtendedInfo;
}

#[derive(Debug, Default, Eq, Parameter, PartialEq)]
#[param(
    name = "id",
    regex = r"(myself)|id (\d+)|(random)|(anonymous)|(the account)|(token identity)|(no one)"
)]
pub enum SomeId {
    Id(u32),
    #[default]
    Myself,
    Anonymous,
    Random,
    Account,
    TokenIdentity,
    NoOne,
}

impl FromStr for SomeId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "myself" => Self::Myself,
            "anonymous" => Self::Anonymous,
            "random" => Self::Random,
            "the account" => Self::Account,
            "token identity" => Self::TokenIdentity,
            "no one" => Self::NoOne,
            id => Self::Id(id.parse().expect("Unable to parse identity id")),
        })
    }
}

impl SomeId {
    pub fn as_address<T: LedgerWorld + AccountWorld>(&self, w: &T) -> Address {
        match self {
            SomeId::Myself => w.setup_id(),
            SomeId::Id(seed) => identity(*seed),
            SomeId::Anonymous => Address::anonymous(),
            SomeId::Random => generate_random_ecdsa_identity().address(),
            SomeId::Account => w.account(),
            // `id1` Address
            SomeId::TokenIdentity => {
                Address::from_str("maffbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wijp").unwrap()
            }
            _ => unimplemented!(),
        }
    }

    pub fn as_maybe_address<T: LedgerWorld + AccountWorld>(&self, w: &T) -> Option<Address> {
        match self {
            SomeId::NoOne => None,
            _ => Some(self.as_address(w)),
        }
    }
}

// TODO: Split or refactor SomeError. It doesn't scale well
#[derive(Debug, Default, Eq, Parameter, PartialEq)]
#[param(
    name = "error",
    regex = "(unauthorized)|(missing permission)|(immutable)|(invalid sender)|(unable to distribute zero)|(partial burn disabled)|(missing funds)|(over maximum)"
)]
pub enum SomeError {
    #[default]
    Unauthorized,
    MissingPermission,
    Immutable,
    InvalidSender,
    UnableToDistributeZero,
    PartialBurnDisabled,
    MissingFunds,
    OverMaximum,
}

impl FromStr for SomeError {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "unauthorized" => Self::Unauthorized,
            "missing permission" => Self::MissingPermission,
            "immutable" => Self::Immutable,
            "invalid sender" => Self::InvalidSender,
            "unable to distribute zero" => Self::UnableToDistributeZero,
            "partial burn disabled" => Self::PartialBurnDisabled,
            "missing funds" => Self::MissingFunds,
            "over maximum" => Self::OverMaximum,
            _ => unimplemented!(),
        })
    }
}
#[derive(Debug, Default, Eq, Parameter, PartialEq)]
#[param(
    name = "permission",
    regex = "(token creation)|(token mint)|(token update)|(token add extended info)|(token remove extended info)"
)]
pub enum SomePermission {
    #[default]
    Create,
    Update,
    AddExtInfo,
    RemoveExtInfo,
    Mint,
    Burn,
}

impl FromStr for SomePermission {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "token creation" => Self::Create,
            "token mint" => Self::Mint,
            "token burn" => Self::Burn,
            "token update" => Self::Update,
            "token add extended info" => Self::AddExtInfo,
            "token remove extended info" => Self::RemoveExtInfo,
            invalid => return Err(format!("Invalid `SomeError`: {invalid}")),
        })
    }
}

impl SomeError {
    pub fn as_many_code(&self) -> ManyErrorCode {
        match self {
            SomeError::Unauthorized => error::unauthorized().code(),
            SomeError::MissingPermission => account::errors::user_needs_role("").code(),
            SomeError::Immutable => {
                ManyError::unknown("Unable to update, this token is immutable").code()
            } // TODO: Custom error
            SomeError::InvalidSender => error::invalid_sender().code(),
            SomeError::UnableToDistributeZero => error::unable_to_distribute_zero("").code(),
            SomeError::PartialBurnDisabled => error::partial_burn_disabled().code(),
            SomeError::MissingFunds => error::missing_funds(Address::anonymous(), "", "").code(),
            SomeError::OverMaximum => error::over_maximum_supply("", "", "").code(),
        }
    }
}

impl SomePermission {
    pub fn as_role(&self) -> Role {
        match self {
            SomePermission::Create => Role::CanTokensCreate,
            SomePermission::Mint => Role::CanTokensMint,
            SomePermission::Burn => Role::CanTokensBurn,
            SomePermission::Update => Role::CanTokensUpdate,
            SomePermission::AddExtInfo => Role::CanTokensAddExtendedInfo,
            SomePermission::RemoveExtInfo => Role::CanTokensRemoveExtendedInfo,
        }
    }
}

pub fn given_token_account<T: LedgerWorld + AccountWorld>(w: &mut T) {
    let sender = w.setup_id();
    let account = AccountModuleBackend::create(
        w.module_impl_mut(),
        &sender,
        CreateArgs {
            description: Some("Token Account".into()),
            roles: None,
            features: FeatureSet::from_iter([
                account::features::tokens::TokenAccountLedger.as_feature()
            ]),
        },
    )
    .expect("Unable to create account");
    *w.account_mut() = account.id
}

pub fn given_account_id_owner<T: LedgerWorld + AccountWorld>(w: &mut T, id: SomeId) {
    let id = id.as_address(w);
    let sender = w.setup_id();
    let account = w.account();
    AccountModuleBackend::add_roles(
        w.module_impl_mut(),
        &sender,
        AddRolesArgs {
            account,
            roles: BTreeMap::from_iter([(id, BTreeSet::from([Role::Owner]))]),
        },
    )
    .expect("Unable to add role to account");

    if id != w.setup_id() {
        let account = w.account();
        AccountModuleBackend::remove_roles(
            w.module_impl_mut(),
            &sender,
            RemoveRolesArgs {
                account,
                roles: BTreeMap::from_iter([(sender, BTreeSet::from([Role::Owner]))]),
            },
        )
        .expect("Unable to remove myself as account owner");
    }
}

pub fn given_account_part_of_can_create<T: LedgerWorld + AccountWorld>(
    w: &mut T,
    id: SomeId,
    permission: SomePermission,
) {
    let id = id.as_address(w);
    let sender = w.setup_id();
    let account = w.account();
    AccountModuleBackend::add_roles(
        w.module_impl_mut(),
        &sender,
        AddRolesArgs {
            account,
            roles: BTreeMap::from([(id, BTreeSet::from_iter([permission.as_role()]))]),
        },
    )
    .expect("Unable to add role to account");
}

fn _create_default_token<T: TokenWorld + LedgerWorld + AccountWorld>(
    w: &mut T,
    id: SomeId,
    max_supply: Option<TokenAmount>,
) {
    let (id, owner) = if let Some(id) = id.as_maybe_address(w) {
        (id, TokenMaybeOwner::Left(id))
    } else {
        (w.setup_id(), TokenMaybeOwner::Right(CborNull))
    };
    let result = LedgerTokensModuleBackend::create(
        w.module_impl_mut(),
        &id,
        crate::default_token_create_args(Some(owner), max_supply),
    )
    .expect("Unable to create default token");
    *w.info_mut() = result.info;
}

pub fn create_default_token_unlimited<T: TokenWorld + LedgerWorld + AccountWorld>(
    w: &mut T,
    id: SomeId,
) {
    _create_default_token(w, id, None);
}

pub fn create_default_token<T: TokenWorld + LedgerWorld + AccountWorld>(w: &mut T, id: SomeId) {
    _create_default_token(w, id, Some(TokenAmount::from(100000000u64)));
}

pub fn verify_error_role<T: LedgerWorld, U: TryInto<Role>>(w: &mut T, role: U)
where
    U::Error: Debug,
{
    let err_addr = Role::try_from(w.error().as_ref().unwrap().argument("role").unwrap()).unwrap();
    assert_eq!(err_addr, role.try_into().unwrap())
}

pub fn verify_error_addr<T: LedgerWorld, U: TryInto<Address>>(w: &mut T, addr: U)
where
    U::Error: Debug,
{
    let err_addr =
        Address::try_from(w.error().as_ref().unwrap().argument("symbol").unwrap()).unwrap();
    assert_eq!(err_addr, addr.try_into().unwrap())
}

pub fn verify_error_code<T: LedgerWorld>(w: &mut T, code: ManyErrorCode) {
    assert_eq!(w.error().as_ref().expect("Expecting an error").code(), code);
}

pub fn refresh_token_info<T: LedgerWorld + TokenWorld>(w: &mut T) {
    let result = LedgerTokensModuleBackend::info(
        w.module_impl(),
        &w.setup_id(),
        TokenInfoArgs {
            symbol: w.info().symbol,
            ..Default::default()
        },
    )
    .expect("Unable to query token info");
    *w.info_mut() = result.info;
    *w.ext_info_mut() = result.extended_info;
}
