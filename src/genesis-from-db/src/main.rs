#![feature(string_remove_matches)]
extern crate core;

use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use many_error::ManyErrorCode;
use many_modules::account::features::multisig::MultisigAccountFeature;
use many_modules::account::features::TryCreateFeature;
use many_modules::account::Account;
use many_types::identity::Address;
use many_types::ledger::{Symbol, TokenAmount, TokenInfo};
use merk::rocksdb;
use merk::rocksdb::{IteratorMode, ReadOptions};
use merk::tree::Tree;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::str::FromStr;
use tracing::{error, Level};
use tracing_subscriber::FmtSubscriber;

const IDSTORE_ROOT: &[u8] = b"/idstore/";
const IDSTORE_SEED_ROOT: &[u8] = b"/config/idstore_seed";
const SYMBOLS_ROOT: &str = "/config/symbols";
const IDENTITY_ROOT: &str = "/config/identity";
const SYMBOLS_ROOT_DASH: &str = const_format::concatcp!(SYMBOLS_ROOT, "/");
const TOKEN_IDENTITY_ROOT: &str = "/config/token_identity";
const NEXT_SUBRESOURCE_ID_ROOT: &str = "/config/subresource_counter/";
const BALANCE_ROOT: &str = "/balances/";
const ACCOUNT_ROOT: &str = "/accounts/";
const ACCOUNT_IDENTITY_ROOT: &str = "/config/account_identity";

// The DATA attribute will be computed at the first block
// pub const DATA_ATTRIBUTES_KEY: &[u8] = b"/data/attributes";
// pub const DATA_INFO_KEY: &[u8] = b"/data/info";

// Do not export EVENTS and MULTISIG, as they are not used in the genesis.
// Token Extended Info won't be imported, as it is not used in the genesis.

// We will activate all migrations from block 0

pub fn key_for_symbol(symbol: &Symbol) -> String {
    format!("/config/symbols/{symbol}")
}

pub fn key_for_account_balance(id: &Address, symbol: &Symbol) -> Vec<u8> {
    format!("/balances/{id}/{symbol}").into_bytes()
}

pub fn key_for_subresource_counter(id: &Address) -> Vec<u8> {
    format!("/config/subresource_counter/{id}").into_bytes()
}

#[derive(Parser)]
struct Opts {
    /// The RocksDB store to load.
    store: PathBuf,
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
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::TRACE)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let Opts { store } = Opts::parse();

    let merk = merk::Merk::open(store).expect("Could not open the store.");

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

// fn extract_subresources(merk: &merk::Merk, token_identity: &Address) {
//     let mut opts = ReadOptions::default();
//     opts.set_iterate_range(rocksdb::PrefixRange(NEXT_SUBRESOURCE_ID_ROOT));
//     let it = merk.iter_opt(IteratorMode::Start, opts);
//
//     // let mut balances = BTreeMap::new();
//     for item in it {
//         let (key, value) = item.expect("Error while reading the DB");
//         let new_v = Tree::decode(key.to_vec(), value.as_ref());
//         let value = new_v.value().to_vec();
//
//         let mut key_string = String::from_utf8(key.to_vec()).expect("Could not decode symbol key");
//         key_string.remove_matches(NEXT_SUBRESOURCE_ID_ROOT);
//         let mut new_data = [0u8; 4];
//         new_data.copy_from_slice(&value);
//         let a = u32::from_be_bytes(new_data);
//         // info!("Subresource: {}", a);
//     }
// }

fn extract_accounts(merk: &merk::Merk) -> AccountJsonRoot {
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
                        error!("Error while reading multisig account: {}", e);
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
