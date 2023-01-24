use crate::storage::account::AccountMeta;
use crate::storage::ledger_tokens::SymbolMeta;
use many_error::ManyError;
use many_identity::Address;
use many_modules::account;
use many_modules::account::features;
use many_modules::account::features::{FeatureInfo, TryCreateFeature};
use many_types::ledger::{Symbol, TokenAmount};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct MultisigFeatureArgJson {
    pub threshold: Option<u64>,
    pub timeout_in_secs: Option<u64>,
    pub execute_automatically: Option<bool>,
}

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct FeatureJson {
    pub id: u32,
    pub arg: Option<serde_json::value::Value>,
}

impl FeatureJson {
    pub fn try_into_feature(&self) -> Option<features::Feature> {
        match self.id {
            features::ledger::AccountLedger::ID => Some(features::Feature::with_id(
                features::ledger::AccountLedger::ID,
            )),
            features::multisig::MultisigAccountFeature::ID => self.arg_into_multisig(),
            _ => None,
        }
    }

    fn arg_into_multisig(&self) -> Option<features::Feature> {
        self.arg.as_ref().map(|a| {
            let s = serde_json::to_string(a).expect("Invalid Feature argument.");
            let a: MultisigFeatureArgJson =
                serde_json::from_str(&s).expect("Invalid Feature argument.");

            features::multisig::MultisigAccountFeature::create(
                a.threshold,
                a.timeout_in_secs,
                a.execute_automatically,
            )
            .as_feature()
        })
    }
}

impl Eq for FeatureJson {}

impl PartialEq<Self> for FeatureJson {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd<Self> for FeatureJson {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl Ord for FeatureJson {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct AccountJson {
    pub id: Option<Address>,
    pub subresource_id: Option<u32>,
    pub description: Option<String>,
    pub roles: BTreeMap<Address, BTreeSet<String>>,
    pub features: BTreeSet<FeatureJson>,
}

/// Converts the JSON Account metadata to our internal representation
impl From<AccountJson> for AccountMeta {
    fn from(value: AccountJson) -> Self {
        Self {
            id: value.id,
            subresource_id: value.subresource_id,
            description: value.description,
            roles: value
                .roles
                .iter()
                .map(|(id, roles)| {
                    (*id, {
                        roles
                            .iter()
                            .map(|s| std::str::FromStr::from_str(s))
                            .collect::<Result<BTreeSet<account::Role>, _>>()
                            .expect("Invalid role.")
                    })
                })
                .collect(),
            features: value
                .features
                .iter()
                .map(|v| v.try_into_feature().expect("Unsupported feature."))
                .collect(),
        }
    }
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct SymbolMetaJson {
    pub name: String,
    pub decimals: u64,
    pub owner: Option<Address>,
    pub maximum: Option<TokenAmount>,
}

/// Converts the JSON Symbol metadata to our internal representation
impl From<SymbolMetaJson> for SymbolMeta {
    fn from(value: SymbolMetaJson) -> Self {
        Self {
            name: value.name,
            decimals: value.decimals,
            owner: value.owner,
            maximum: value.maximum,
        }
    }
}

/// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Clone, Debug, Default)]
pub struct InitialStateJson {
    pub identity: Address,
    pub initial: BTreeMap<Address, BTreeMap<String, TokenAmount>>,
    pub token_identity: Option<Address>,
    pub account_identity: Option<Address>,
    pub token_next_subresource: Option<u32>,
    pub symbols: BTreeMap<Address, String>,
    pub symbols_meta: Option<BTreeMap<Address, SymbolMetaJson>>,
    pub accounts: Option<Vec<AccountJson>>,
    pub id_store_seed: Option<u64>,
    pub id_store_keys: Option<BTreeMap<String, String>>,
    pub hash: Option<String>,
}

impl InitialStateJson {
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(Box::new)?;
        let s = json5::from_str(&content).map_err(Box::new)?;
        Ok(s)
    }

    pub fn symbols(&self) -> BTreeMap<Address, String> {
        self.symbols.clone()
    }

    pub fn balances(&self) -> Result<BTreeMap<Address, BTreeMap<Symbol, TokenAmount>>, ManyError> {
        self.initial
            .iter()
            .map(|(id, b)| {
                let mut balances = BTreeMap::new();
                for (token_name, amount) in b {
                    let symbol = self
                        .symbols
                        .iter()
                        .find_map(|(s, n)| {
                            if *s == token_name.as_str() || n == token_name {
                                Some(*s)
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| {
                            ManyError::unknown(format!("Could not resolve symbol '{token_name}'"))
                        })?;
                    balances.insert(symbol, amount.clone());
                }
                Ok((*id, balances))
            })
            .collect()
    }
}
