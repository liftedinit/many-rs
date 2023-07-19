#![feature(string_remove_matches)]
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
use many_modules::account::features::multisig::MultisigAccountFeature;
use many_modules::account::features::TryCreateFeature;
use many_modules::account::Account;
use many_modules::events::EventLog;
use many_types::identity::Address;
use many_types::ledger::{TokenAmount, TokenInfo};
use merk::rocksdb;
use merk::rocksdb::{IteratorMode, ReadOptions};
use merk::tree::Tree;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::str::FromStr;
use tracing::{trace, Level};
use tracing_subscriber::FmtSubscriber;
use many_ledger::storage::multisig::MultisigTransactionStorage;

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

#[derive(serde_derive::Serialize)]
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

    match extract {
        Extract::Genesis => extract_genesis(&merk),
        Extract::Events => extract_events(&merk),
        Extract::Multisig => extract_multisig(&merk),
    };
}

fn extract_genesis(merk: &merk::Merk) {
    let idstore_root = extract_idstore(&merk);
    let symbols_root = extract_symbols(&merk);
    let balances_root = extract_balances(&merk);
    let identity_root = extract_identity(&merk);
    let token_identity_root = extract_token_identity(&merk);
    let account_identity_root = extract_account_identity(&merk);
    let accounts_root = extract_accounts(&merk);

    let mega = CombinedJson {
        id_store: idstore_root,
        symbols: symbols_root,
        balances: balances_root,
        identity: identity_root,
        token_identity: token_identity_root,
        account_identity: account_identity_root,
        accounts: accounts_root,
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&mega).expect("Could not serialize"),
    );
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

fn extract_events(merk: &merk::Merk) {
    const EVENTS_ROOT: &str = "/events/";

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(EVENTS_ROOT));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    for item in it {
        let (key, value) = item.expect("Error while reading the DB");
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        let event_log: EventLog = minicbor::decode(&value).expect("Could not decode event log");

        println!("{:?}", event_log);
    }
}

fn extract_multisig(merk: &merk::Merk) {
    const MULTISIG_ROOT: &str = "/multisig/";

    let mut opts = ReadOptions::default();
    opts.set_iterate_range(rocksdb::PrefixRange(MULTISIG_ROOT));
    let it = merk.iter_opt(IteratorMode::Start, opts);

    for item in it {
        let (key, value) = item.expect("Error while reading the DB");
        let new_v = Tree::decode(key.to_vec(), value.as_ref());
        let value = new_v.value().to_vec();

        let event_log: MultisigTransactionStorage = minicbor::decode(&value).expect("Could not decode multisig log");

        println!("{:?}", event_log);
    }
}