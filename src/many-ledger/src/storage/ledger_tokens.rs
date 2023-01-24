use crate::error;
use crate::migration::tokens::TOKEN_MIGRATION;
use crate::storage::iterator::LedgerIterator;
use crate::storage::{key_for_account_balance, LedgerStorage, IDENTITY_ROOT, SYMBOLS_ROOT};
use many_error::ManyError;
use many_identity::Address;
use many_modules::events::EventInfo;
use many_modules::ledger::extended_info::{ExtendedInfoKey, TokenExtendedInfo};
use many_modules::ledger::{
    TokenAddExtendedInfoArgs, TokenAddExtendedInfoReturns, TokenCreateArgs, TokenCreateReturns,
    TokenInfoArgs, TokenInfoReturns, TokenRemoveExtendedInfoArgs, TokenRemoveExtendedInfoReturns,
    TokenUpdateArgs, TokenUpdateReturns,
};
use many_types::ledger::{Symbol, TokenAmount, TokenInfo, TokenInfoSummary, TokenInfoSupply};
use many_types::{AttributeRelatedIndex, Either, SortOrder};
use merk::{BatchEntry, Op};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

pub const SYMBOLS_ROOT_DASH: &str = const_format::concatcp!(SYMBOLS_ROOT, "/");
pub const TOKEN_IDENTITY_ROOT: &str = "/config/token_identity";
pub const TOKEN_SUBRESOURCE_COUNTER_ROOT: &str = "/config/token_subresource_id";

pub fn key_for_symbol(symbol: &Symbol) -> String {
    format!("/config/symbols/{symbol}")
}

pub fn key_for_ext_info(symbol: &Symbol) -> Vec<u8> {
    format!("/config/ext_info/{symbol}").into_bytes()
}

pub struct SymbolMeta {
    pub name: String,
    pub decimals: u64,
    pub owner: Option<Address>,
    pub maximum: Option<TokenAmount>,
}

pub fn verify_tokens_sender(sender: &Address, token_identity: Address) -> Result<(), ManyError> {
    if *sender != token_identity {
        return Err(error::invalid_sender());
    }
    Ok(())
}

impl LedgerStorage {
    #[inline]
    fn _total_supply(
        initial_balance: BTreeMap<Address, BTreeMap<Symbol, TokenAmount>>,
    ) -> Result<BTreeMap<Symbol, TokenAmount>, ManyError> {
        let mut total_supply = BTreeMap::new();
        for v in initial_balance.into_values() {
            for (symbol, tokens) in v.into_iter() {
                *total_supply.entry(symbol).or_default() += tokens;
            }
        }
        Ok(total_supply)
    }

    #[inline]
    fn _token_info(
        symbol: Symbol,
        ticker: String,
        meta: SymbolMeta,
        total_supply: TokenAmount,
    ) -> TokenInfo {
        TokenInfo {
            symbol,
            summary: TokenInfoSummary {
                name: meta.name,
                ticker,
                decimals: meta.decimals,
            },
            supply: TokenInfoSupply {
                total: total_supply.clone(),
                circulating: total_supply,
                maximum: meta.maximum,
            },
            owner: meta.owner,
        }
    }

    /// Add token-related config to the persistent storage if the Token Migration is active
    /// Note: This will change storage hash
    pub fn with_tokens(
        mut self,
        symbols: &BTreeMap<Symbol, String>,
        symbols_meta: Option<BTreeMap<Symbol, SymbolMeta>>,
        token_identity: Option<Address>,
        token_next_subresource: Option<u32>,
        initial_balances: BTreeMap<Address, BTreeMap<Symbol, TokenAmount>>,
    ) -> Result<Self, ManyError> {
        if self.migrations.is_active(&TOKEN_MIGRATION) {
            let symbols_meta = symbols_meta
                .ok_or_else(|| ManyError::unknown("Symbols metadata needs to be provided"))?; // TODO: Custom error
            let total_supply = LedgerStorage::_total_supply(initial_balances)?;

            for symbol in symbols.keys() {
                if !symbols_meta.contains_key(symbol) {
                    return Err(ManyError::unknown(format!(
                        "Symbol {symbol} missing metadata"
                    ))); // TODO: Custom error
                }

                if !total_supply.contains_key(symbol) {
                    return Err(ManyError::unknown(format!(
                        "Symbol {symbol} missing total supply"
                    ))); // TODO: Custom error
                }
            }

            let mut batch: Vec<BatchEntry> = Vec::new();

            for (k, meta) in symbols_meta.into_iter() {
                let total_supply = total_supply[&k].clone(); // Safe
                let ticker = symbols[&k].clone(); // Safe
                let info = LedgerStorage::_token_info(k, ticker, meta, total_supply.clone());

                batch.push((
                    key_for_ext_info(&k),
                    Op::Put(
                        minicbor::to_vec(TokenExtendedInfo::default())
                            .map_err(ManyError::serialization_error)?,
                    ),
                ));
                batch.push((
                    key_for_symbol(&k).into(),
                    Op::Put(minicbor::to_vec(info).map_err(ManyError::serialization_error)?),
                ));
            }

            batch.push((
                TOKEN_IDENTITY_ROOT.as_bytes().to_vec(),
                Op::Put(
                    token_identity
                        .unwrap_or(self.get_identity(IDENTITY_ROOT)?)
                        .to_vec(),
                ),
            ));
            batch.push((
                TOKEN_SUBRESOURCE_COUNTER_ROOT.as_bytes().to_vec(),
                Op::Put(token_next_subresource.unwrap_or(0).to_be_bytes().to_vec()),
            ));

            self.persistent_store
                .apply(batch.as_slice())
                .map_err(error::storage_apply_failed)?;

            self.commit_storage()?;
        }

        Ok(self)
    }

    pub(crate) fn get_owner(&self, symbol: &Symbol) -> Result<Option<Address>, ManyError> {
        let token_info_enc = self
            .persistent_store
            .get(key_for_symbol(symbol).as_bytes())
            .map_err(error::storage_get_failed)?
            .ok_or_else(|| error::token_info_not_found(symbol))?;

        let info: TokenInfo =
            minicbor::decode(&token_info_enc).map_err(ManyError::deserialization_error)?;

        Ok(info.owner)
    }

    /// Fetch symbols from `/config/symbols/{symbol}`
    ///     No CBOR decoding needed.
    pub(crate) fn _get_symbols(&self) -> Result<BTreeSet<Symbol>, ManyError> {
        let mut symbols = BTreeSet::new();
        let it = LedgerIterator::all_symbols(&self.persistent_store, SortOrder::Indeterminate);
        for item in it {
            let (k, _) = item.map_err(ManyError::unknown)?;
            symbols.insert(Symbol::from_str(
                std::str::from_utf8(&k.as_ref()[SYMBOLS_ROOT_DASH.len()..])
                    .map_err(ManyError::deserialization_error)?, // TODO: We could safely use from_utf8_unchecked() if performance is an issue
            )?);
        }
        Ok(symbols)
    }

    pub fn get_token_info_summary(&self) -> Result<BTreeMap<Symbol, TokenInfoSummary>, ManyError> {
        let mut info_summary = BTreeMap::new();
        if self.migrations.is_active(&TOKEN_MIGRATION) {
            let it = LedgerIterator::all_symbols(&self.persistent_store, SortOrder::Indeterminate);
            for item in it {
                let (k, v) = item.map_err(ManyError::unknown)?;
                let info: TokenInfo =
                    minicbor::decode(&v).map_err(ManyError::deserialization_error)?;
                info_summary.insert(
                    Symbol::from_str(
                        std::str::from_utf8(&k.as_ref()[SYMBOLS_ROOT_DASH.len()..])
                            .map_err(ManyError::deserialization_error)?, // TODO: We could safely use from_utf8_unchecked() if performance is an issue
                    )?,
                    info.summary,
                );
            }
        } else {
            tracing::warn!("`get_token_info_summary()` called while TOKEN_MIGRATION is NOT active. Returning empty TokenInfoSummary.")
        }
        Ok(info_summary)
    }

    fn update_symbols(&mut self, symbol: Symbol, ticker: String) -> Result<(), ManyError> {
        let mut symbols = self.get_symbols_and_tickers()?;
        symbols.insert(symbol, ticker);

        self.persistent_store
            .apply(&[(
                b"/config/symbols".to_vec(),
                Op::Put(minicbor::to_vec(&symbols).map_err(ManyError::serialization_error)?),
            )])
            .map_err(error::storage_apply_failed)?;

        Ok(())
    }

    pub fn create_token(
        &mut self,
        sender: &Address,
        args: TokenCreateArgs,
    ) -> Result<TokenCreateReturns, ManyError> {
        let TokenCreateArgs {
            summary,
            owner,
            initial_distribution,
            maximum_supply,
            extended_info,
            memo,
        } = args;

        // Create a new token symbol and store in memory and in the persistent store
        let symbol =
            self.get_next_subresource(TOKEN_IDENTITY_ROOT, TOKEN_SUBRESOURCE_COUNTER_ROOT)?;
        self.update_symbols(symbol, summary.ticker.clone())?;

        // Initialize the total supply following the initial token distribution, if any
        let mut batch: Vec<BatchEntry> = Vec::new();
        let total_supply = if let Some(ref initial_distribution) = initial_distribution {
            let mut total_supply = TokenAmount::zero();
            for (k, v) in initial_distribution {
                let key = key_for_account_balance(k, &symbol);
                batch.push((key, Op::Put(v.to_vec())));
                total_supply += v.clone();
            }
            total_supply
        } else {
            TokenAmount::zero()
        };

        let supply = TokenInfoSupply {
            total: total_supply.clone(),
            circulating: total_supply,
            maximum: maximum_supply.clone(),
        };

        // Create the token information and store it in the persistent storage
        let maybe_owner = owner.clone().map_or(Some(*sender), Either::left);
        let info = TokenInfo {
            symbol,
            summary: summary.clone(),
            supply,
            owner: maybe_owner,
        };
        batch.push((
            key_for_symbol(&symbol).into(),
            Op::Put(minicbor::to_vec(&info).map_err(ManyError::serialization_error)?),
        ));

        let ext_info = extended_info
            .clone()
            .map_or(TokenExtendedInfo::default(), |e| e);
        batch.push((
            key_for_ext_info(&symbol),
            Op::Put(minicbor::to_vec(&ext_info).map_err(ManyError::serialization_error)?),
        ));

        self.log_event(EventInfo::TokenCreate {
            summary,
            symbol,
            owner,
            initial_distribution,
            maximum_supply,
            extended_info,
            memo,
        })?;

        batch.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
        self.persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        self.maybe_commit()?;

        Ok(TokenCreateReturns { info })
    }

    pub fn info_token(&self, args: TokenInfoArgs) -> Result<TokenInfoReturns, ManyError> {
        let TokenInfoArgs {
            symbol,
            extended_info,
        } = args;

        // Try fetching the token info from the persistent storage
        let token_info_enc = self
            .persistent_store
            .get(key_for_symbol(&symbol).as_bytes())
            .map_err(ManyError::unknown)?
            .ok_or_else(|| error::token_info_not_found(symbol))?;

        let ext_info_enc = self
            .persistent_store
            .get(&key_for_ext_info(&symbol))
            .map_err(error::storage_get_failed)?
            .ok_or_else(|| error::ext_info_not_found(symbol))?;

        let mut ext_info: TokenExtendedInfo =
            minicbor::decode(&ext_info_enc).map_err(ManyError::deserialization_error)?;

        let ext_info = if let Some(indices) = extended_info {
            ext_info.retain(indices)?;
            ext_info
        } else {
            ext_info
        };

        let info: TokenInfo =
            minicbor::decode(&token_info_enc).map_err(ManyError::deserialization_error)?;

        Ok(TokenInfoReturns {
            info,
            extended_info: ext_info,
        })
    }

    pub fn update_token(
        &mut self,
        _sender: &Address,
        args: TokenUpdateArgs,
    ) -> Result<TokenUpdateReturns, ManyError> {
        let TokenUpdateArgs {
            symbol,
            name,
            ticker,
            decimals,
            owner,
            memo,
        } = args;

        // Try fetching the token info from the persistent storage
        if let Some(enc) = self
            .persistent_store
            .get(key_for_symbol(&symbol).as_bytes())
            .map_err(ManyError::unknown)?
        {
            let mut info: TokenInfo = minicbor::decode(&enc).unwrap();

            if let Some(name) = name.as_ref() {
                info.summary.name = name.clone();
            }
            if let Some(ticker) = ticker.as_ref() {
                self.update_symbols(symbol, ticker.clone())?;
                info.summary.ticker = ticker.clone();
            }
            if let Some(decimals) = decimals {
                info.summary.decimals = decimals;
            }
            match owner.as_ref() {
                None => {}
                Some(x) => match x {
                    Either::Left(addr) => info.owner = Some(*addr),
                    Either::Right(_) => info.owner = None,
                },
            };

            self.persistent_store
                .apply(&[(
                    key_for_symbol(&symbol).into(),
                    Op::Put(minicbor::to_vec(&info).map_err(ManyError::serialization_error)?),
                )])
                .map_err(error::storage_apply_failed)?;

            self.log_event(EventInfo::TokenUpdate {
                symbol,
                name,
                ticker,
                decimals,
                owner,
                memo,
            })?;

            self.maybe_commit()?;
        } else {
            return Err(ManyError::unknown(format!(
                "Symbol {symbol} not found in persistent storage"
            )));
        }
        Ok(TokenUpdateReturns {})
    }

    pub fn add_extended_info(
        &mut self,
        args: TokenAddExtendedInfoArgs,
    ) -> Result<TokenAddExtendedInfoReturns, ManyError> {
        let TokenAddExtendedInfoArgs {
            symbol,
            extended_info,
            memo,
        } = args;

        // Fetch existing extended info, if any
        let mut ext_info = if let Some(ext_info_enc) = self
            .persistent_store
            .get(&key_for_ext_info(&symbol))
            .map_err(error::storage_get_failed)?
        {
            minicbor::decode(&ext_info_enc).map_err(ManyError::deserialization_error)?
        } else {
            TokenExtendedInfo::new()
        };

        let mut indices = vec![];
        if let Some(memo) = extended_info.memo() {
            ext_info = ext_info.with_memo(memo.clone())?;
            indices.push(AttributeRelatedIndex::from(ExtendedInfoKey::Memo));
        }
        if let Some(logos) = extended_info.visual_logo() {
            ext_info = ext_info.with_visual_logo(logos.clone())?;
            indices.push(AttributeRelatedIndex::from(ExtendedInfoKey::VisualLogo));
        }

        self.persistent_store
            .apply(&[(
                key_for_ext_info(&symbol),
                Op::Put(minicbor::to_vec(&ext_info).map_err(ManyError::serialization_error)?),
            )])
            .map_err(error::storage_apply_failed)?;

        self.log_event(EventInfo::TokenAddExtendedInfo {
            symbol,
            extended_info: indices,
            memo,
        })?;

        self.maybe_commit()?;

        Ok(TokenAddExtendedInfoReturns {})
    }

    pub fn remove_extended_info(
        &mut self,
        args: TokenRemoveExtendedInfoArgs,
    ) -> Result<TokenRemoveExtendedInfoReturns, ManyError> {
        let TokenRemoveExtendedInfoArgs {
            symbol,
            extended_info,
            memo,
        } = args;

        // Fetch existing extended info, if any
        let ext_info_enc = self
            .persistent_store
            .get(&key_for_ext_info(&symbol))
            .map_err(error::storage_get_failed)?
            .ok_or_else(|| error::ext_info_not_found(symbol))?;

        let mut ext_info: TokenExtendedInfo =
            minicbor::decode(&ext_info_enc).map_err(ManyError::deserialization_error)?;

        for index in &extended_info {
            if ext_info.contains_index(index)? {
                ext_info.remove(index)?;
            }
        }

        self.persistent_store
            .apply(&[(
                key_for_ext_info(&symbol),
                Op::Put(minicbor::to_vec(&ext_info).map_err(ManyError::serialization_error)?),
            )])
            .map_err(error::storage_apply_failed)?;

        self.log_event(EventInfo::TokenRemoveExtendedInfo {
            symbol,
            extended_info,
            memo,
        })?;

        self.maybe_commit()?;

        Ok(TokenRemoveExtendedInfoReturns {})
    }
}
