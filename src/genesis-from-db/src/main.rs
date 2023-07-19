#![feature(string_remove_matches)]
#![feature(box_into_inner)]
/// This is a tool to extract the genesis data from a RocksDB store.
/// This tool requires the persistent storage to have the following migrations activated:
/// - Data
/// - Memo
/// - Token
///
/// This tool will extract
/// - The IDStore seed
/// - The IDStore keys
/// - The symbols
/// - The token identity
/// - The account identity
/// - The balances
/// - The accounts
/// and create a genesis file, i.e., `ledger_state.json`.
///
/// This tool will NOT extract
/// - The data attributes (recalculated at block 1)
/// - The data info (recalculated at block 1)
/// - The events (not used in the genesis)
/// - The multisig (not used in the genesis)
/// - The token extended info (not used in the genesis)
/// - The next subresource id (recalculated)
///
/// In our context, we will need to activate the following migrations from block 0
/// - Data migration
/// - Memo migration
/// - Token migration
extern crate core;

use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use many_error::{ManyError, ManyErrorCode};
use many_ledger::storage::multisig::MultisigTransactionStorage;
use many_modules::account::features::multisig::{MultisigAccountFeature, MultisigTransactionState};
use many_modules::account::features::TryCreateFeature;
use many_modules::account::{Account, Role};
use many_modules::events::{AccountMultisigTransaction, EventInfo, EventLog};
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_types::identity::Address;
use many_types::ledger::{
    LedgerTokensAddressMap, TokenAmount, TokenInfo, TokenInfoSummary, TokenMaybeOwner,
};
use merk::rocksdb;
use merk::rocksdb::{IteratorMode, ReadOptions};
use merk::tree::Tree;
use serde_derive::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::UNIX_EPOCH;
use tracing::{trace, Level};
use tracing_subscriber::FmtSubscriber;

enum Extract {
    Genesis,
    Events,
    Multisig,
}

// Implement the `FromStr` trait for `Extract`.
impl FromStr for Extract {
    type Err = ManyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "genesis" => Ok(Extract::Genesis),
            "events" => Ok(Extract::Events),
            "multisig" => Ok(Extract::Multisig),
            _ => Err(ManyError::unknown("Invalid extract type")),
        }
    }
}

#[derive(Parser)]
struct Opts {
    /// The RocksDB store to load.
    store: PathBuf,

    /// What to extract from the persistent storage.
    extract: Extract,
}

#[derive(serde_derive::Serialize)]
struct IdStoreJsonRoot {
    id_store_seed: u64,
    id_store_keys: BTreeMap<String, String>,
}

#[derive(serde_derive::Serialize)]
pub struct SymbolMeta {
    pub name: String,
    pub decimals: u64,
    pub owner: Option<Address>,
    pub maximum: Option<TokenAmount>,
}

#[derive(serde_derive::Serialize)]
struct SymbolsJsonRoot {
    symbols: BTreeMap<Address, String>,
    symbols_meta: BTreeMap<String, SymbolMeta>,
}

#[derive(serde_derive::Serialize)]
struct BalancesJsonRoot {
    initial: BTreeMap<String, BTreeMap<String, TokenAmount>>,
}

#[derive(serde_derive::Serialize)]
struct IdentityJsonRoot {
    identity: Address,
}

#[derive(serde_derive::Serialize)]
struct TokenIdentityJsonRoot {
    token_identity: Address,
}

#[derive(serde_derive::Serialize)]
struct AccountIdentityJsonRoot {
    account_identity: Address,
}

#[derive(Debug, serde_derive::Serialize)]
struct FeatureJson {
    id: u32,
    arg: Option<serde_json::value::Value>,
}

#[derive(serde_derive::Serialize)]
struct AccountJsonRoot {
    accounts: Vec<AccountJsonParamRoot>,
}

#[derive(serde_derive::Serialize)]
struct AccountJsonParamRoot {
    subresource_id: u32,
    id: Address,
    description: Option<String>,
    roles: BTreeMap<Address, BTreeSet<String>>,
    features: Vec<FeatureJson>,
}

#[derive(serde_derive::Serialize)]
struct CombinedJson {
    #[serde(flatten)]
    id_store: IdStoreJsonRoot,

    #[serde(flatten)]
    symbols: SymbolsJsonRoot,

    #[serde(flatten)]
    balances: BalancesJsonRoot,

    #[serde(flatten)]
    identity: IdentityJsonRoot,

    #[serde(flatten)]
    token_identity: TokenIdentityJsonRoot,

    #[serde(flatten)]
    account_identity: AccountIdentityJsonRoot,

    #[serde(flatten)]
    accounts: AccountJsonRoot,
}

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let Opts { store, extract } = Opts::parse();

    let merk = merk::Merk::open(store).expect("Could not open the store.");

    let to_print = match extract {
        Extract::Genesis => extract_genesis(&merk),
        Extract::Events => extract_events(&merk),
        Extract::Multisig => extract_multisig(&merk),
    };

    println!("{to_print}");
}

fn extract_genesis(merk: &merk::Merk) -> String {
    let idstore_root = extract_idstore(merk);
    let symbols_root = extract_symbols(merk);
    let balances_root = extract_balances(merk);
    let identity_root = extract_identity(merk);
    let token_identity_root = extract_token_identity(merk);
    let account_identity_root = extract_account_identity(merk);
    let accounts_root = extract_accounts(merk);

    let mega = CombinedJson {
        id_store: idstore_root,
        symbols: symbols_root,
        balances: balances_root,
        identity: identity_root,
        token_identity: token_identity_root,
        account_identity: account_identity_root,
        accounts: accounts_root,
    };

    serde_json::to_string_pretty(&mega).expect("Could not serialize")
}

fn extract_idstore(merk: &merk::Merk) -> IdStoreJsonRoot {
    const IDSTORE_ROOT: &[u8] = b"/idstore/";
    const IDSTORE_SEED_ROOT: &[u8] = b"/config/idstore_seed";

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(IDSTORE_ROOT));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    let mut idstore = BTreeMap::new();
    for item in it {
        let (key, value) = item.expect("Error while reading the DB");
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        idstore.insert(
            general_purpose::STANDARD.encode(key.as_ref()),
            general_purpose::STANDARD.encode(value),
        );
    }

    IdStoreJsonRoot {
        id_store_seed: merk
            .get(IDSTORE_SEED_ROOT)
            .expect("Could not read seed")
            .map_or(0u64, |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                u64::from_be_bytes(bytes)
            }),
        id_store_keys: idstore,
    }
}

fn extract_symbols(merk: &merk::Merk) -> SymbolsJsonRoot {
    const SYMBOLS_ROOT: &str = "/config/symbols";
    const SYMBOLS_ROOT_DASH: &str = const_format::concatcp!(SYMBOLS_ROOT, "/");

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(SYMBOLS_ROOT_DASH));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    let mut symbols = BTreeMap::new();
    for item in it {
        let (key, value) = item.expect("Error while reading the DB");
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();
        let info =
            minicbor::decode::<TokenInfo>(value.as_ref()).expect("Could not decode token info");
        let mut key_string = String::from_utf8(key.to_vec()).expect("Could not decode symbol key");
        key_string.remove_matches(SYMBOLS_ROOT_DASH);

        symbols.insert(
            key_string,
            SymbolMeta {
                name: info.summary.name,
                decimals: info.summary.decimals,
                owner: info.owner,
                maximum: info.supply.maximum,
            },
        );
    }
    let symbols_root = merk
        .get(SYMBOLS_ROOT.as_ref())
        .expect("Could not read symbols root")
        .expect("Could not read symbols root value");
    let symbols_map: BTreeMap<Address, String> =
        minicbor::decode(symbols_root.as_ref()).expect("Could not decode symbols root");
    SymbolsJsonRoot {
        symbols: symbols_map,
        symbols_meta: symbols,
    }
}

fn extract_balances(merk: &merk::Merk) -> BalancesJsonRoot {
    const BALANCE_ROOT: &str = "/balances/";

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(BALANCE_ROOT));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    let mut balances = BTreeMap::new();
    for item in it {
        let (key, value) = item.expect("Error while reading the DB");
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        let mut key_string = String::from_utf8(key.to_vec()).expect("Could not decode symbol key");
        key_string.remove_matches(BALANCE_ROOT);
        let (id, _) = key_string.split_once('/').expect("Could not split key");
        let amount = TokenAmount::from(value);

        // TODO: Support more than just MFX
        balances.insert(
            id.to_string(),
            BTreeMap::from([("MFX".to_string(), amount)]),
        );
    }
    BalancesJsonRoot { initial: balances }
}

fn extract_identity(merk: &merk::Merk) -> IdentityJsonRoot {
    const IDENTITY_ROOT: &str = "/config/identity";

    IdentityJsonRoot {
        identity: Address::from_bytes(
            &merk
                .get(IDENTITY_ROOT.as_bytes())
                .expect("Could not read identity")
                .expect("Could not read identity value"),
        )
        .expect("Could not decode identity"),
    }
}

fn extract_token_identity(merk: &merk::Merk) -> TokenIdentityJsonRoot {
    const TOKEN_IDENTITY_ROOT: &str = "/config/token_identity";

    TokenIdentityJsonRoot {
        token_identity: Address::from_bytes(
            &merk
                .get(TOKEN_IDENTITY_ROOT.as_bytes())
                .expect("Could not read identity")
                .expect("Could not read identity value"),
        )
        .expect("Could not decode identity"),
    }
}

fn extract_account_identity(merk: &merk::Merk) -> AccountIdentityJsonRoot {
    const ACCOUNT_IDENTITY_ROOT: &str = "/config/account_identity";

    AccountIdentityJsonRoot {
        account_identity: Address::from_bytes(
            &merk
                .get(ACCOUNT_IDENTITY_ROOT.as_bytes())
                .expect("Could not read identity")
                .expect("Could not read identity value"),
        )
        .expect("Could not decode identity"),
    }
}

fn extract_accounts(merk: &merk::Merk) -> AccountJsonRoot {
    const ACCOUNT_ROOT: &str = "/accounts/";

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(ACCOUNT_ROOT));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    let mut accounts = vec![];
    for item in it {
        let (key, value) = item.expect("Error while reading the DB");
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        let mut key_string = String::from_utf8(key.to_vec()).expect("Could not decode symbol key");
        key_string.remove_matches(ACCOUNT_ROOT);

        let acc = minicbor::decode::<Account>(&value).expect("Could not decode account");
        let id = Address::from_str(&key_string).expect("Could not decode address");
        let subresource_id = id.subresource_id();
        let description = acc.description;

        let mut roles = BTreeMap::new();
        for (addr, role) in acc.roles {
            roles.insert(
                addr,
                role.into_iter()
                    .map(|r| r.to_string())
                    .collect::<BTreeSet<String>>(),
            );
        }

        let mut acc_features = vec![];
        for feature in acc.features.iter() {
            let feature_id = feature.id();

            // The only feature currently supporting arguments if the multisig account feature
            let arg = MultisigAccountFeature::try_create(feature);
            match arg {
                Ok(arg) => acc_features.push(FeatureJson {
                    id: feature_id,
                    arg: Some(json!({
                        "threshold": arg.arg.threshold,
                        "timeout_in_secs": arg.arg.timeout_in_secs,
                        "execute_automatically": arg.arg.execute_automatically,
                    })),
                }),
                Err(e) => {
                    // This is not a multisig account feature
                    if e.code() != ManyErrorCode::AttributeNotFound {
                        trace!("Error while reading multisig account: {}", e);
                    }

                    // At this point we know that this is not a multisig account feature but some other feature with no arguments
                    acc_features.push(FeatureJson {
                        id: feature_id,
                        arg: None,
                    })
                }
            }
        }

        accounts.push(AccountJsonParamRoot {
            subresource_id: subresource_id.unwrap(),
            id,
            description,
            roles,
            features: acc_features,
        });
    }
    AccountJsonRoot { accounts }
}

#[derive(Debug, Serialize)]
struct SendTransactionJson {
    pub from: Option<Address>,
    pub to: Address,
    pub amount: TokenAmount,
    pub symbol: Address,
    pub memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct AccountCreateTransactionJson {
    pub description: Option<String>,
    pub roles: Option<AddressRoleMapJson>,
    pub features: Vec<FeatureJson>,
}

#[derive(Debug, Serialize)]
enum MultisigTransactionJson {
    Send(SendTransactionJson),
    AccountCreate(AccountCreateTransactionJson),
    AccountSetDescription(AccountSetDescriptionTransactionJson),
    AccountAddRoles(AccountAddRolesTransactionJson),
    AccountRemoveRoles(AccountRemoveRolesTransactionJson),
    AccountDisable(AccountDisableTransactionJson),
    AccountAddFeatures(AccountAddFeaturesTransactionJson),
    AccountMultisigSubmit(AccountMultisigSubmitTransactionJson),
    AccountMultisigApprove(AccountMultisigApproveTransactionJson),
    AccountMultisigRevoke(AccountMultisigRevokeTransactionJson),
    AccountMultisigExecute(AccountMultisigExecuteTransactionJson),
    AccountMultisigWithdraw(AccountMultisigWithdrawTransactionJson),
    AccountMultisigSetDefaults(AccountMultisigSetDefaultsTransactionJson),
    TokenCreate(TokenCreateTransactionJson),
    TokenUpdate(TokenUpdateTransactionJson),
    // TokenAddExtendedInfo(TokenAddExtendedInfoTransactionJson),
    // TokenRemoveExtendedInfo(TokenRemoveExtendedInfoTransactionJson),
    TokenMint(TokenMintTransactionJson),
    TokenBurn(TokenBurnTransactionJson),
}

#[derive(Debug, Serialize)]
struct AccountMultisigApproveTransactionJson {
    token: String,
}

#[derive(Debug, Serialize)]
struct AccountMultisigRevokeTransactionJson {
    token: String,
}

#[derive(Debug, Serialize)]
struct AccountMultisigExecuteTransactionJson {
    token: String,
}

#[derive(Debug, Serialize)]
struct AccountMultisigWithdrawTransactionJson {
    token: String,
}

#[derive(Debug, Serialize)]
struct AccountMultisigSetDefaultsTransactionJson {
    account: Address,
    threshold: Option<u64>,
    timeout_in_secs: Option<u64>,
    execure_automatically: Option<bool>,
}

#[derive(Debug, Serialize)]
struct TokenCreateTransactionJson {
    summary: TokenInfoSummaryJson,
    owner: Option<TokenMaybeOwnerJson>,
    initial_distribution: Option<LedgerTokensAddressMap>,
    maximum_supply: Option<TokenAmount>,
    extended_info: Option<TokenExtendedInfoJson>,
    memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenInfoSummaryJson {
    name: String,
    ticker: String,
    decimals: u64,
}

#[derive(Debug, Serialize)]
enum EitherJson {
    Left(Address),
    Right(Option<()>),
}

#[derive(Debug, Serialize)]
struct TokenMaybeOwnerJson(EitherJson);

#[derive(Debug, Serialize)]
struct TokenExtendedInfoJson();

// Implement From TokenExtendedInfo for TokenExtendedInfoJson
impl From<TokenExtendedInfo> for TokenExtendedInfoJson {
    fn from(_info: TokenExtendedInfo) -> Self {
        TokenExtendedInfoJson()
    }
}

// Implement From TokenInfoSummary for TokenInfoSummaryJson
impl From<TokenInfoSummary> for TokenInfoSummaryJson {
    fn from(summary: TokenInfoSummary) -> Self {
        TokenInfoSummaryJson {
            name: summary.name,
            ticker: summary.ticker,
            decimals: summary.decimals,
        }
    }
}

// Implement From TokenMaybeOwner for TokenMaybeOwnerJson
impl From<TokenMaybeOwner> for TokenMaybeOwnerJson {
    fn from(owner: TokenMaybeOwner) -> Self {
        match owner {
            TokenMaybeOwner::Left(addr) => Self(EitherJson::Left(addr)),
            TokenMaybeOwner::Right(_) => Self(EitherJson::Right(None)),
        }
    }
}

#[derive(Debug, Serialize)]
struct TokenUpdateTransactionJson {
    symbol: Address,
    name: Option<String>,
    ticker: Option<String>,
    decimals: Option<u64>,
    owner: Option<TokenMaybeOwnerJson>,
    memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenAddExtendedInfoTransactionJson();

#[derive(Debug, Serialize)]
struct TokenRemoveExtendedInfoTransactionJson();

#[derive(Debug, Serialize)]
struct TokenMintTransactionJson {
    symbol: Address,
    distribution: LedgerTokensAddressMap,
    memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenBurnTransactionJson {
    symbol: Address,
    distribution: LedgerTokensAddressMap,
    memo: Option<String>,
    error_on_under_burn: Option<bool>,
}

// Implement From AccountMultisigTrqansaction for MultisigTransactionJson
impl From<AccountMultisigTransaction> for MultisigTransactionJson {
    fn from(tx: AccountMultisigTransaction) -> Self {
        match tx {
            AccountMultisigTransaction::Send(args) => {
                let memo = if let Some(memo) = args.memo {
                    if memo.len() == 1 {
                        memo.iter_str().next().map(String::from)
                    } else {
                        None
                    }
                } else {
                    None
                };

                MultisigTransactionJson::Send(SendTransactionJson {
                    from: args.from,
                    to: args.to,
                    amount: args.amount,
                    symbol: args.symbol,
                    memo,
                })
            }
            AccountMultisigTransaction::AccountCreate(args) => {
                let mut acc_features = vec![];
                for feature in args.features.iter() {
                    let feature_id = feature.id();

                    // The only feature currently supporting arguments if the multisig account feature
                    let arg = MultisigAccountFeature::try_create(feature);
                    match arg {
                        Ok(arg) => acc_features.push(FeatureJson {
                            id: feature_id,
                            arg: Some(json!({
                                "threshold": arg.arg.threshold,
                                "timeout_in_secs": arg.arg.timeout_in_secs,
                                "execute_automatically": arg.arg.execute_automatically,
                            })),
                        }),
                        Err(e) => {
                            // This is not a multisig account feature
                            if e.code() != ManyErrorCode::AttributeNotFound {
                                trace!("Error while reading multisig account: {}", e);
                            }

                            // At this point we know that this is not a multisig account feature but some other feature with no arguments
                            acc_features.push(FeatureJson {
                                id: feature_id,
                                arg: None,
                            })
                        }
                    }
                }

                MultisigTransactionJson::AccountCreate(AccountCreateTransactionJson {
                    description: args.description,
                    roles: args.roles.map(|k| {
                        k.into_iter()
                            .map(|(k, v)| (k, v.into_iter().map(|v| v.into()).collect()))
                            .collect()
                    }),
                    features: acc_features,
                })
            }
            AccountMultisigTransaction::AccountSetDescription(args) => {
                MultisigTransactionJson::AccountSetDescription(
                    AccountSetDescriptionTransactionJson {
                        account: args.account,
                        description: args.description,
                    },
                )
            }
            AccountMultisigTransaction::AccountAddRoles(args) => {
                MultisigTransactionJson::AccountAddRoles(AccountAddRolesTransactionJson {
                    account: args.account,
                    roles: args
                        .roles
                        .into_iter()
                        .map(|(k, v)| (k, v.into_iter().map(|v| v.into()).collect()))
                        .collect(),
                })
            }
            AccountMultisigTransaction::AccountRemoveRoles(args) => {
                MultisigTransactionJson::AccountRemoveRoles(AccountRemoveRolesTransactionJson {
                    account: args.account,
                    roles: args
                        .roles
                        .into_iter()
                        .map(|(k, v)| (k, v.into_iter().map(|v| v.into()).collect()))
                        .collect(),
                })
            }
            AccountMultisigTransaction::AccountDisable(args) => {
                MultisigTransactionJson::AccountDisable(AccountDisableTransactionJson {
                    account: args.account,
                })
            }
            AccountMultisigTransaction::AccountAddFeatures(args) => {
                let mut features = vec![];
                for feature in args.features.iter() {
                    let feature_id = feature.id();

                    // The only feature currently supporting arguments if the multisig account feature
                    let arg = MultisigAccountFeature::try_create(feature);
                    match arg {
                        Ok(arg) => features.push(FeatureJson {
                            id: feature_id,
                            arg: Some(json!({
                                "threshold": arg.arg.threshold,
                                "timeout_in_secs": arg.arg.timeout_in_secs,
                                "execute_automatically": arg.arg.execute_automatically,
                            })),
                        }),
                        Err(e) => {
                            // This is not a multisig account feature
                            if e.code() != ManyErrorCode::AttributeNotFound {
                                trace!("Error while reading multisig account: {}", e);
                            }

                            // At this point we know that this is not a multisig account feature but some other feature with no arguments
                            features.push(FeatureJson {
                                id: feature_id,
                                arg: None,
                            })
                        }
                    }
                }

                MultisigTransactionJson::AccountAddFeatures(AccountAddFeaturesTransactionJson {
                    account: args.account,
                    roles: args.roles.map(|k| {
                        k.into_iter()
                            .map(|(k, v)| (k, v.into_iter().map(|v| v.into()).collect()))
                            .collect()
                    }),
                    features,
                })
            }
            AccountMultisigTransaction::AccountMultisigSubmit(args) => {
                let memo_ = args.memo_.map(|memo| memo.to_string());
                let data_ = args.data_.map(|data| hex::encode(data.as_bytes()));
                let transaction = Box::new(
                    Box::<AccountMultisigTransaction>::into_inner(args.transaction).into(),
                );
                let memo = if let Some(memo) = args.memo {
                    if memo.len() == 1 {
                        memo.iter_str().next().map(String::from)
                    } else {
                        None
                    }
                } else {
                    None
                };
                MultisigTransactionJson::AccountMultisigSubmit(
                    AccountMultisigSubmitTransactionJson {
                        account: args.account,
                        memo_,
                        transaction,
                        threshold: args.threshold,
                        timeout_in_secs: args.timeout_in_secs,
                        execute_automatically: args.execute_automatically,
                        data_,
                        memo,
                    },
                )
            }
            AccountMultisigTransaction::AccountMultisigApprove(args) => {
                MultisigTransactionJson::AccountMultisigApprove(
                    AccountMultisigApproveTransactionJson {
                        token: hex::encode(args.token.to_vec()),
                    },
                )
            }
            AccountMultisigTransaction::AccountMultisigRevoke(args) => {
                MultisigTransactionJson::AccountMultisigRevoke(
                    AccountMultisigRevokeTransactionJson {
                        token: hex::encode(args.token.to_vec()),
                    },
                )
            }
            AccountMultisigTransaction::AccountMultisigExecute(args) => {
                MultisigTransactionJson::AccountMultisigExecute(
                    AccountMultisigExecuteTransactionJson {
                        token: hex::encode(args.token.to_vec()),
                    },
                )
            }
            AccountMultisigTransaction::AccountMultisigWithdraw(args) => {
                MultisigTransactionJson::AccountMultisigWithdraw(
                    AccountMultisigWithdrawTransactionJson {
                        token: hex::encode(args.token.to_vec()),
                    },
                )
            }
            AccountMultisigTransaction::AccountMultisigSetDefaults(args) => {
                MultisigTransactionJson::AccountMultisigSetDefaults(
                    AccountMultisigSetDefaultsTransactionJson {
                        account: args.account,
                        threshold: args.threshold,
                        timeout_in_secs: args.timeout_in_secs,
                        execure_automatically: args.execute_automatically,
                    },
                )
            }
            AccountMultisigTransaction::TokenCreate(args) => {
                let memo = if let Some(memo) = args.memo {
                    if memo.len() == 1 {
                        memo.iter_str().next().map(String::from)
                    } else {
                        None
                    }
                } else {
                    None
                };
                MultisigTransactionJson::TokenCreate(TokenCreateTransactionJson {
                    summary: args.summary.into(),
                    owner: args.owner.map(|owner| owner.into()),
                    initial_distribution: args.initial_distribution,
                    maximum_supply: args.maximum_supply,
                    extended_info: args.extended_info.map(|extended_info| extended_info.into()), // FIXME: We don't care about ExtInfo
                    memo,
                })
            }
            AccountMultisigTransaction::TokenUpdate(args) => {
                let memo = if let Some(memo) = args.memo {
                    if memo.len() == 1 {
                        memo.iter_str().next().map(String::from)
                    } else {
                        None
                    }
                } else {
                    None
                };
                MultisigTransactionJson::TokenUpdate(TokenUpdateTransactionJson {
                    symbol: args.symbol,
                    name: args.name,
                    ticker: args.ticker,
                    decimals: args.decimals,
                    owner: args.owner.map(|owner| owner.into()),
                    memo,
                })
            }
            // AccountMultisigTransaction::TokenAddExtendedInfo(args) => {
            //     MultisigTransactionJson::TokenAddExtendedInfo(TokenAddExtendedInfoTransactionJson()) // FIXME: We don't care about ExtInfo
            // }
            // AccountMultisigTransaction::TokenRemoveExtendedInfo(args) => {
            //     MultisigTransactionJson::TokenRemoveExtendedInfo(TokenRemoveExtendedInfoTransactionJson()) // FIXME: We don't care about ExtInfo
            // }
            AccountMultisigTransaction::TokenMint(args) => {
                let memo = if let Some(memo) = args.memo {
                    if memo.len() == 1 {
                        memo.iter_str().next().map(String::from)
                    } else {
                        None
                    }
                } else {
                    None
                };
                MultisigTransactionJson::TokenMint(TokenMintTransactionJson {
                    symbol: args.symbol,
                    distribution: args.distribution,
                    memo,
                })
            }
            AccountMultisigTransaction::TokenBurn(args) => {
                let memo = if let Some(memo) = args.memo {
                    if memo.len() == 1 {
                        memo.iter_str().next().map(String::from)
                    } else {
                        None
                    }
                } else {
                    None
                };
                MultisigTransactionJson::TokenBurn(TokenBurnTransactionJson {
                    symbol: args.symbol,
                    distribution: args.distribution,
                    memo,
                    error_on_under_burn: args.error_on_under_burn,
                })
            }
            _ => todo!(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ApproverInfoJson {
    pub approved: bool,
}

#[derive(Debug, Serialize)]
struct MultisigTransactionInfoJson {
    pub memo_: Option<String>,
    pub transaction: MultisigTransactionJson,
    pub submitter: Address,
    pub approvers: BTreeMap<Address, ApproverInfoJson>,
    pub threshold: u64,
    pub execute_automatically: bool,
    pub timeout: u64,
    pub data_: Option<String>, // Hex encoded
    pub state: MultisigTransactionStateJson,
    pub memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct MultisigTransactionStorageJson {
    pub account: Address,
    pub info: MultisigTransactionInfoJson,
    pub creation: u64,
    pub disabled: bool,
}

#[derive(Debug, Serialize)]
enum EventInfoJson {
    Send(SendEventJson),
    AccountCreate(AccountCreateEventJson),
    AccountSetDescription(AccountSetDescriptionEventJson),
    AccountAddRoles(AccountAddRolesEventJson),
    AccountRemoveRoles(AccountRemoveRolesEventJson),
    AccountDisable(AccountDisableEventJson),
    AccountAddFeatures(AccountAddFeaturesEventJson),
    AccountMultisigSubmit(AccountMultisigSubmitEventJson),
    AccountMultisigApprove(AccountMultisigApproveEventJson),
    AccountMultisigRevoke(AccountMultisigRevokeEventJson),
    AccountMultisigExecute(AccountMultisigExecuteEventJson),
    AccountMultisigWithdraw(AccountMultisigWithdrawEventJson),
    AccountMultisigSetDefaults(AccountMultisigSetDefaultsEventJson),
    AccountMultisigExpired(AccountMultisigExpiredEventJson),
    TokenCreate(TokenCreateEventJson),
    TokenUpdate(TokenUpdateEventJson),
    TokenMint(TokenMintEventJson),
    TokenBurn(TokenBurnEventJson),
}

#[derive(Debug, Serialize)]
struct SendEventJson {
    from: Address,
    to: Address,
    symbol: Address,
    amount: TokenAmount,
    memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct AccountCreateEventJson {
    account: Address,
    description: Option<String>,
    roles: AddressRoleMapJson,
    features: Vec<FeatureJson>,
}

#[derive(Debug, Serialize)]
struct AccountSetDescriptionEventJson {
    account: Address,
    description: String,
}

#[derive(Debug, Serialize)]
struct AccountAddRolesEventJson {
    account: Address,
    roles: AddressRoleMapJson,
}

#[derive(Debug, Serialize)]
struct AccountRemoveRolesEventJson {
    account: Address,
    roles: AddressRoleMapJson,
}

#[derive(Debug, Serialize)]
struct AccountDisableEventJson {
    account: Address,
}

#[derive(Debug, Serialize)]
struct AccountAddFeaturesEventJson {
    account: Address,
    roles: AddressRoleMapJson,
    features: Vec<FeatureJson>,
}

#[derive(Debug, Serialize)]
struct AccountMultisigSubmitEventJson {
    submitter: Address,
    account: Address,
    memo_: Option<String>,
    transaction: Box<MultisigTransactionJson>,
    token: Option<String>,
    threshold: u64,
    timeout: u64,
    execute_automatically: bool,
    data: Option<String>,
    memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct AccountMultisigApproveEventJson {
    account: Address,
    token: String,
    approver: Address,
}

#[derive(Debug, Serialize)]
struct AccountMultisigRevokeEventJson {
    account: Address,
    token: String,
    revoker: Address,
}

#[derive(Debug, Serialize)]
struct AccountMultisigExecuteEventJson {
    account: Address,
    token: String,
    executer: Option<Address>,
    response: String,
}

#[derive(Debug, Serialize)]
struct AccountMultisigWithdrawEventJson {
    account: Address,
    token: String,
    withdrawer: Address,
}

#[derive(Debug, Serialize)]
struct AccountMultisigSetDefaultsEventJson {
    submitter: Address,
    account: Address,
    threshold: Option<u64>,
    timeout_in_secs: Option<u64>,
    execute_automatically: Option<bool>,
}

#[derive(Debug, Serialize)]
struct AccountMultisigExpiredEventJson {
    account: Address,
    token: String,
    time: u64,
}

#[derive(Debug, Serialize)]
struct TokenCreateEventJson {
    summary: TokenInfoSummaryJson,
    symbol: Address,
    owner: Option<TokenMaybeOwnerJson>,
    initial_distribution: Option<LedgerTokensAddressMap>,
    maximum_supply: Option<TokenAmount>,
    extended_info: Option<TokenExtendedInfoJson>,
    memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenUpdateEventJson {
    symbol: Address,
    name: Option<String>,
    ticker: Option<String>,
    decimals: Option<u64>,
    owner: Option<TokenMaybeOwnerJson>,
    memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenMintEventJson {
    symbol: Address,
    distribution: LedgerTokensAddressMap,
    memo: Option<String>,
}

#[derive(Debug, Serialize)]
struct TokenBurnEventJson {
    symbol: Address,
    distribution: LedgerTokensAddressMap,
    memo: Option<String>,
}

// Implement From EventInfo for EventInfoJson
impl From<EventInfo> for EventInfoJson {
    fn from(e: EventInfo) -> Self {
        match e {
            EventInfo::Send {
                from,
                to,
                symbol,
                amount,
                memo,
            } => Self::Send(SendEventJson {
                from,
                to,
                symbol,
                amount,
                memo: memo.map(|m| {
                    m.iter_str()
                        .next()
                        .map(String::from)
                        .expect("Only string memo are supported...")
                }),
            }),
            EventInfo::AccountCreate {
                account,
                description,
                roles,
                features,
            } => {
                let mut acc_features = vec![];
                for feature in features.iter() {
                    let feature_id = feature.id();

                    // The only feature currently supporting arguments if the multisig account feature
                    let arg = MultisigAccountFeature::try_create(feature);
                    match arg {
                        Ok(arg) => acc_features.push(FeatureJson {
                            id: feature_id,
                            arg: Some(json!({
                                "threshold": arg.arg.threshold,
                                "timeout_in_secs": arg.arg.timeout_in_secs,
                                "execute_automatically": arg.arg.execute_automatically,
                            })),
                        }),
                        Err(e) => {
                            // This is not a multisig account feature
                            if e.code() != ManyErrorCode::AttributeNotFound {
                                trace!("Error while reading multisig account: {}", e);
                            }

                            // At this point we know that this is not a multisig account feature but some other feature with no arguments
                            acc_features.push(FeatureJson {
                                id: feature_id,
                                arg: None,
                            })
                        }
                    }
                }
                Self::AccountCreate(AccountCreateEventJson {
                    account,
                    description,
                    roles: roles
                        .into_iter()
                        .map(|(k, v)| (k, v.into_iter().map(|v| v.into()).collect()))
                        .collect(),
                    features: acc_features,
                })
            }
            EventInfo::AccountSetDescription {
                account,
                description,
            } => Self::AccountSetDescription(AccountSetDescriptionEventJson {
                account,
                description,
            }),
            EventInfo::AccountAddRoles { account, roles } => {
                Self::AccountAddRoles(AccountAddRolesEventJson {
                    account,
                    roles: roles
                        .into_iter()
                        .map(|(k, v)| (k, v.into_iter().map(|v| v.into()).collect()))
                        .collect(),
                })
            }
            EventInfo::AccountRemoveRoles { account, roles } => {
                Self::AccountRemoveRoles(AccountRemoveRolesEventJson {
                    account,
                    roles: roles
                        .into_iter()
                        .map(|(k, v)| (k, v.into_iter().map(|v| v.into()).collect()))
                        .collect(),
                })
            }
            EventInfo::AccountDisable { account } => {
                Self::AccountDisable(AccountDisableEventJson { account })
            }
            EventInfo::AccountAddFeatures {
                account,
                roles,
                features,
            } => {
                let mut acc_features = vec![];
                for feature in features.iter() {
                    let feature_id = feature.id();

                    // The only feature currently supporting arguments if the multisig account feature
                    let arg = MultisigAccountFeature::try_create(feature);
                    match arg {
                        Ok(arg) => acc_features.push(FeatureJson {
                            id: feature_id,
                            arg: Some(json!({
                                "threshold": arg.arg.threshold,
                                "timeout_in_secs": arg.arg.timeout_in_secs,
                                "execute_automatically": arg.arg.execute_automatically,
                            })),
                        }),
                        Err(e) => {
                            // This is not a multisig account feature
                            if e.code() != ManyErrorCode::AttributeNotFound {
                                trace!("Error while reading multisig account: {}", e);
                            }

                            // At this point we know that this is not a multisig account feature but some other feature with no arguments
                            acc_features.push(FeatureJson {
                                id: feature_id,
                                arg: None,
                            })
                        }
                    }
                }
                Self::AccountAddFeatures(AccountAddFeaturesEventJson {
                    account,
                    roles: roles
                        .into_iter()
                        .map(|(k, v)| (k, v.into_iter().map(|v| v.into()).collect()))
                        .collect(),
                    features: acc_features,
                })
            }
            EventInfo::AccountMultisigSubmit {
                submitter,
                account,
                memo_,
                transaction,
                token,
                threshold,
                timeout,
                execute_automatically,
                data_,
                memo,
            } => Self::AccountMultisigSubmit(AccountMultisigSubmitEventJson {
                submitter,
                account,
                memo_: memo_.map(|m| m.to_string()),
                transaction: Box::new(
                    Box::<AccountMultisigTransaction>::into_inner(transaction).into(),
                ),
                token: token.map(|t| hex::encode(t.to_vec())),
                threshold,
                timeout: timeout.secs(),
                execute_automatically,
                data: data_.map(|d| hex::encode(d.as_bytes())),
                memo: memo.map(|m| {
                    m.iter_str()
                        .next()
                        .map(String::from)
                        .expect("Only string memo are supported...")
                }),
            }),
            EventInfo::AccountMultisigApprove {
                account,
                token,
                approver,
            } => Self::AccountMultisigApprove(AccountMultisigApproveEventJson {
                account,
                token: hex::encode(token.to_vec()),
                approver,
            }),
            EventInfo::AccountMultisigRevoke {
                account,
                token,
                revoker,
            } => Self::AccountMultisigRevoke(AccountMultisigRevokeEventJson {
                account,
                token: hex::encode(token.to_vec()),
                revoker,
            }),
            EventInfo::AccountMultisigExecute {
                account,
                token,
                executer,
                response,
            } => Self::AccountMultisigExecute(AccountMultisigExecuteEventJson {
                account,
                token: hex::encode(token.to_vec()),
                executer,
                response: hex::encode(
                    minicbor::to_vec(response).expect("Failed to serialize response"),
                ),
            }),
            EventInfo::AccountMultisigWithdraw {
                account,
                token,
                withdrawer,
            } => Self::AccountMultisigWithdraw(AccountMultisigWithdrawEventJson {
                account,
                token: hex::encode(token.to_vec()),
                withdrawer,
            }),
            EventInfo::AccountMultisigSetDefaults {
                submitter,
                account,
                threshold,
                timeout_in_secs,
                execute_automatically,
            } => Self::AccountMultisigSetDefaults(AccountMultisigSetDefaultsEventJson {
                submitter,
                account,
                threshold,
                timeout_in_secs,
                execute_automatically,
            }),
            EventInfo::AccountMultisigExpired {
                account,
                token,
                time,
            } => Self::AccountMultisigExpired(AccountMultisigExpiredEventJson {
                account,
                token: hex::encode(token.to_vec()),
                time: time.secs(),
            }),
            EventInfo::TokenCreate {
                summary,
                symbol,
                owner,
                initial_distribution,
                maximum_supply,
                extended_info,
                memo,
            } => {
                Self::TokenCreate(TokenCreateEventJson {
                    summary: summary.into(),
                    symbol,
                    owner: owner.map(|owner| owner.into()),
                    initial_distribution,
                    maximum_supply,
                    extended_info: extended_info.map(|extended_info| extended_info.into()), // FIXME: We don't care about ExtInfo
                    memo: memo.map(|m| {
                        m.iter_str()
                            .next()
                            .map(String::from)
                            .expect("Only string memo are supported...")
                    }),
                })
            }
            EventInfo::TokenUpdate {
                symbol,
                name,
                ticker,
                decimals,
                owner,
                memo,
            } => Self::TokenUpdate(TokenUpdateEventJson {
                symbol,
                name,
                ticker,
                decimals,
                owner: owner.map(|owner| owner.into()),
                memo: memo.map(|m| {
                    m.iter_str()
                        .next()
                        .map(String::from)
                        .expect("Only string memo are supported...")
                }),
            }),
            EventInfo::TokenMint {
                symbol,
                distribution,
                memo,
            } => Self::TokenMint(TokenMintEventJson {
                symbol,
                distribution,
                memo: memo.map(|m| {
                    m.iter_str()
                        .next()
                        .map(String::from)
                        .expect("Only string memo are supported...")
                }),
            }),
            EventInfo::TokenBurn {
                symbol,
                distribution,
                memo,
            } => Self::TokenBurn(TokenBurnEventJson {
                symbol,
                distribution,
                memo: memo.map(|m| {
                    m.iter_str()
                        .next()
                        .map(String::from)
                        .expect("Only string memo are supported...")
                }),
            }),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Serialize)]
enum MultisigTransactionStateJson {
    Pending = 0,
    ExecutedAutomatically,
    ExecutedManually,
    Withdrawn,
    Expired,
}

#[derive(Debug, Serialize)]
struct EventLogJson {
    pub id: String, // Hex encoded
    pub time: u64,
    pub content: EventInfoJson,
}

// Implement From EventLog for EventLogJson
impl From<EventLog> for EventLogJson {
    fn from(e: EventLog) -> Self {
        Self {
            id: hex::encode(e.id.as_ref()),
            time: e.time.secs(),
            content: e.content.into(),
        }
    }
}

#[derive(Debug, Ord, Eq, PartialEq, PartialOrd, Serialize, strum_macros::Display)]
#[repr(u8)]
#[strum(serialize_all = "camelCase")]
pub enum RoleJson {
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

// Implement From Role for RoleJson
impl From<Role> for RoleJson {
    fn from(r: Role) -> Self {
        match r {
            Role::Owner => RoleJson::Owner,
            Role::CanLedgerTransact => RoleJson::CanLedgerTransact,
            Role::CanMultisigSubmit => RoleJson::CanMultisigSubmit,
            Role::CanMultisigApprove => RoleJson::CanMultisigApprove,
            Role::CanKvStorePut => RoleJson::CanKvStorePut,
            Role::CanKvStoreDisable => RoleJson::CanKvStoreDisable,
            Role::CanKvStoreTransfer => RoleJson::CanKvStoreTransfer,
            Role::CanTokensCreate => RoleJson::CanTokensCreate,
            Role::CanTokensMint => RoleJson::CanTokensMint,
            Role::CanTokensBurn => RoleJson::CanTokensBurn,
            Role::CanTokensUpdate => RoleJson::CanTokensUpdate,
            Role::CanTokensAddExtendedInfo => RoleJson::CanTokensAddExtendedInfo,
            Role::CanTokensRemoveExtendedInfo => RoleJson::CanTokensRemoveExtendedInfo,
        }
    }
}

pub type AddressRoleMapJson = BTreeMap<Address, BTreeSet<RoleJson>>;

#[derive(Debug, Serialize)]
struct AccountSetDescriptionTransactionJson {
    account: Address,
    description: String,
}

#[derive(Debug, Serialize)]
struct AccountAddRolesTransactionJson {
    account: Address,
    roles: AddressRoleMapJson,
}

#[derive(Debug, Serialize)]
struct AccountRemoveRolesTransactionJson {
    account: Address,
    roles: AddressRoleMapJson,
}

#[derive(Debug, Serialize)]
struct AccountDisableTransactionJson {
    account: Address,
}

#[derive(Debug, Serialize)]
struct AccountAddFeaturesTransactionJson {
    account: Address,
    roles: Option<AddressRoleMapJson>,
    features: Vec<FeatureJson>,
}

#[derive(Debug, Serialize)]
struct AccountMultisigSubmitTransactionJson {
    pub account: Address,
    pub memo_: Option<String>,
    pub transaction: Box<MultisigTransactionJson>,
    pub threshold: Option<u64>,
    pub timeout_in_secs: Option<u64>,
    pub execute_automatically: Option<bool>,
    pub data_: Option<String>, // Hex encoded
    pub memo: Option<String>,
}

// Implement From MultisigTransactionStorage for MultisigTransactionStorageJson
impl From<MultisigTransactionStorage> for MultisigTransactionStorageJson {
    fn from(m: MultisigTransactionStorage) -> Self {
        let info = m.info;
        let mut approvers = BTreeMap::new();
        for (addr, approver) in info.approvers {
            approvers.insert(
                addr,
                ApproverInfoJson {
                    approved: approver.approved,
                },
            );
        }

        let state = match info.state {
            MultisigTransactionState::Pending => MultisigTransactionStateJson::Pending,
            MultisigTransactionState::ExecutedAutomatically => {
                MultisigTransactionStateJson::ExecutedAutomatically
            }
            MultisigTransactionState::ExecutedManually => {
                MultisigTransactionStateJson::ExecutedManually
            }
            MultisigTransactionState::Withdrawn => MultisigTransactionStateJson::Withdrawn,
            MultisigTransactionState::Expired => MultisigTransactionStateJson::Expired,
        };

        let memo_ = info.memo_.map(|memo| memo.to_string());
        let memo = if let Some(memo) = info.memo {
            if memo.len() == 1 {
                memo.iter_str().next().map(String::from)
            } else {
                None
            }
        } else {
            None
        };
        let data_ = info.data_.map(|data| hex::encode(data.as_bytes()));

        let transaction = info.transaction.into();

        MultisigTransactionStorageJson {
            account: m.account,
            info: MultisigTransactionInfoJson {
                memo_,
                transaction,
                submitter: info.submitter,
                approvers,
                threshold: info.threshold,
                execute_automatically: info.execute_automatically,
                timeout: info.timeout.secs(),
                data_,
                state,
                memo,
            },
            creation: m.creation.duration_since(UNIX_EPOCH).unwrap().as_secs(),
            disabled: m.disabled,
        }
    }
}

fn extract_events(merk: &merk::Merk) -> String {
    const EVENTS_ROOT: &str = "/events/";

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(EVENTS_ROOT));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    let mut events = BTreeMap::new();
    for item in it {
        let (key, value) = item.expect("Error while reading the DB");
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        let event_log: EventLog = minicbor::decode(&value).expect("Could not decode event log");

        let event_log_json = EventLogJson::from(event_log);

        events.insert(hex::encode(key), event_log_json);
    }
    serde_json::to_string_pretty(&events).expect("Could not serialize")
}

fn extract_multisig(merk: &merk::Merk) -> String {
    const MULTISIG_ROOT: &str = "/multisig/";

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(MULTISIG_ROOT));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    let mut multisig_logs = BTreeMap::new();
    for item in it {
        let (key, value) = item.expect("Error while reading the DB");
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        let multisig_log: MultisigTransactionStorage =
            minicbor::decode(&value).expect("Could not decode multisig log");

        let multisig_log_json = MultisigTransactionStorageJson::from(multisig_log);

        multisig_logs.insert(hex::encode(key), multisig_log_json);
    }
    serde_json::to_string_pretty(&multisig_logs).expect("Could not serialize")
}
