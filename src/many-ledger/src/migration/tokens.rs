use crate::error;
use crate::migration::MIGRATIONS;
use crate::storage::account::{ACCOUNT_IDENTITY_ROOT, ACCOUNT_SUBRESOURCE_ID_ROOT};
use crate::storage::ledger_tokens::{key_for_ext_info, key_for_symbol, TOKEN_IDENTITY_ROOT};
use crate::storage::{key_for_subresource_counter, InnerStorage, IDENTITY_ROOT, SYMBOLS_ROOT};
use linkme::distributed_slice;
use many_error::ManyError;
use many_identity::Address;
use many_migration::InnerMigration;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_types::ledger::{Symbol, TokenInfo, TokenInfoSummary, TokenInfoSupply};
use merk::{BatchEntry, Op};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

fn migrate_account_identity(storage: &mut merk::Merk) -> Result<(), ManyError> {
    // Fetch the root identity
    let root_identity = storage
        .get(IDENTITY_ROOT.as_bytes())
        .map_err(error::storage_get_failed)?
        .ok_or_else(|| error::storage_key_not_found(IDENTITY_ROOT))?;
    let subresource_counter = storage
        .get(ACCOUNT_SUBRESOURCE_ID_ROOT.as_bytes())
        .map_err(error::storage_get_failed)?
        .map_or(0u32, |x| {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(x.as_slice());
            u32::from_be_bytes(bytes)
        });
    let root_address = Address::from_bytes(&root_identity)?;

    let batch: Vec<BatchEntry> = vec![
        (ACCOUNT_SUBRESOURCE_ID_ROOT.as_bytes().to_vec(), Op::Delete),
        (
            ACCOUNT_IDENTITY_ROOT.as_bytes().to_vec(),
            Op::Put(root_identity),
        ),
        (
            key_for_subresource_counter(&root_address, true),
            Op::Put(subresource_counter.to_be_bytes().to_vec()),
        ),
    ];

    // And use it as the account identity
    storage.apply(&batch).map_err(error::storage_apply_failed)?;

    Ok(())
}

fn migrate_token(
    storage: &mut merk::Merk,
    extra: &HashMap<String, Value>,
) -> Result<(), ManyError> {
    // Make sure we have all the parameters we need for this migration
    let params = [
        "token_identity",
        "token_next_subresource",
        "symbol",
        "symbol_name",
        "symbol_decimals",
        "symbol_total",
        "symbol_circulating",
        "symbol_maximum",
        "symbol_owner",
    ];
    for param in params {
        if !extra.contains_key(param) {
            return Err(ManyError::unknown(format!(
                "Missing extra parameter '{param}' for Token Migration"
            )));
        }
    }

    let token_identity: String = serde_json::from_value(extra["token_identity"].clone())
        .map_err(ManyError::deserialization_error)?;
    let token_identity = Address::from_str(&token_identity)?;

    let token_next_subresource: u32 =
        serde_json::from_value(extra["token_next_subresource"].clone())
            .map_err(ManyError::deserialization_error)?;

    let symbol: String = serde_json::from_value(extra["symbol"].clone())
        .map_err(ManyError::deserialization_error)?;
    let symbol = Symbol::from_str(&symbol)?;

    // Get symbol list from DB
    let symbol_and_ticker_enc = storage
        .get(SYMBOLS_ROOT.as_bytes())
        .map_err(error::storage_get_failed)?
        .ok_or_else(|| error::storage_key_not_found(SYMBOLS_ROOT))?;

    let symbol_and_ticker: BTreeMap<Address, String> =
        minicbor::decode(&symbol_and_ticker_enc).map_err(ManyError::deserialization_error)?;

    // Get the symbol ticker from symbol list
    let ticker = symbol_and_ticker
        .get(&symbol)
        .ok_or_else(|| ManyError::unknown(format!("Symbol {symbol} not found in DB")))
        .cloned()?;

    let info = (move || {
        Ok::<_, serde_json::Error>(TokenInfo {
            symbol,
            summary: TokenInfoSummary {
                name: serde_json::from_value(extra["symbol_name"].clone())?,
                ticker,
                decimals: serde_json::from_value(extra["symbol_decimals"].clone())?,
            },
            supply: TokenInfoSupply {
                total: serde_json::from_value(extra["symbol_total"].clone())?,
                circulating: serde_json::from_value(extra["symbol_circulating"].clone())?,
                maximum: serde_json::from_value(extra["symbol_maximum"].clone())?,
            },
            owner: serde_json::from_value(extra["symbol_owner"].clone())?,
        })
    })()
    .map_err(ManyError::deserialization_error)?;

    let batch: Vec<BatchEntry> = vec![
        (
            key_for_ext_info(&symbol),
            Op::Put(
                minicbor::to_vec(TokenExtendedInfo::default())
                    .map_err(ManyError::serialization_error)?,
            ),
        ),
        (
            key_for_subresource_counter(&token_identity, true),
            Op::Put(token_next_subresource.to_be_bytes().to_vec()),
        ),
        (
            key_for_symbol(&symbol).into_bytes(),
            Op::Put(minicbor::to_vec(info).map_err(ManyError::serialization_error)?),
        ),
        (
            TOKEN_IDENTITY_ROOT.as_bytes().to_vec(),
            Op::Put(token_identity.to_vec()),
        ),
    ];

    storage
        .apply(batch.as_slice())
        .map_err(error::storage_apply_failed)?;

    Ok(())
}

fn initialize(storage: &mut InnerStorage, extra: &HashMap<String, Value>) -> Result<(), ManyError> {
    migrate_account_identity(storage)?;
    migrate_token(storage, extra)?;

    Ok(())
}

#[distributed_slice(MIGRATIONS)]
pub static TOKEN_MIGRATION: InnerMigration<InnerStorage, ManyError> =
    InnerMigration::new_initialize(
        initialize,
        "Token Migration",
        "Move the database to new subresource counter and new token metadata",
    );
