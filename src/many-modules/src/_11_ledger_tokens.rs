use crate::EmptyReturn;
use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;
use many_types::{cbor_type_decl, ledger, AttributeRelatedIndex, Memo};
use minicbor::{Decode, Encode};

pub mod extended_info;

cbor_type_decl!(
    pub struct TokenCreateArgs {
        0 => summary: ledger::TokenInfoSummary,
        1 => owner: Option<ledger::TokenMaybeOwner>,
        2 => initial_distribution: Option<ledger::LedgerTokensAddressMap>,
        3 => maximum_supply: Option<ledger::TokenAmount>,
        4 => extended_info: Option<extended_info::TokenExtendedInfo>,
        5 => memo: Option<Memo>,
    }

    pub struct TokenCreateReturns {
        0 => info: ledger::TokenInfo,
    }

    pub struct TokenInfoArgs {
        0 => symbol: ledger::Symbol,
        1 => extended_info: Option<Vec<AttributeRelatedIndex>>, // TODO: This thing should be of at least length 1
    }

    pub struct TokenInfoReturns {
        0 => info: ledger::TokenInfo,
        1 => extended_info: extended_info::TokenExtendedInfo,
    }

    pub struct TokenUpdateArgs {
        0 => symbol: ledger::Symbol,
        1 => name: Option<String>,
        2 => ticker: Option<String>,
        3 => decimals: Option<u64>,
        4 => owner: Option<ledger::TokenMaybeOwner>,
        5 => memo: Option<Memo>,
    }

    pub struct TokenAddExtendedInfoArgs {
        0 => symbol: ledger::Symbol,
        1 => extended_info: extended_info::TokenExtendedInfo,
        2 => memo: Option<Memo>,
    }

    pub struct TokenRemoveExtendedInfoArgs {
        0 => symbol: ledger::Symbol,
        1 => extended_info: Vec<AttributeRelatedIndex>, // TODO: This thing should be of at least length 1
        2 => memo: Option<Memo>,
    }
);

pub type TokenUpdateReturns = EmptyReturn;
pub type TokenAddExtendedInfoReturns = EmptyReturn;
pub type TokenRemoveExtendedInfoReturns = EmptyReturn;

#[many_module(name = LedgerTokensModule, id = 11, namespace = tokens, many_modules_crate = crate)]
#[cfg_attr(test, mockall::automock)]
pub trait LedgerTokensModuleBackend: Send {
    #[many(deny_anonymous)]
    fn create(
        &mut self,
        sender: &Address,
        args: TokenCreateArgs,
    ) -> Result<TokenCreateReturns, ManyError>;

    fn info(&self, sender: &Address, args: TokenInfoArgs) -> Result<TokenInfoReturns, ManyError>;

    #[many(deny_anonymous)]
    fn update(
        &mut self,
        sender: &Address,
        args: TokenUpdateArgs,
    ) -> Result<TokenUpdateReturns, ManyError>;

    #[many(deny_anonymous)]
    fn add_extended_info(
        &mut self,
        sender: &Address,
        args: TokenAddExtendedInfoArgs,
    ) -> Result<TokenAddExtendedInfoReturns, ManyError>;

    #[many(deny_anonymous)]
    fn remove_extended_info(
        &mut self,
        sender: &Address,
        args: TokenRemoveExtendedInfoArgs,
    ) -> Result<TokenRemoveExtendedInfoReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::extended_info::TokenExtendedInfo;
    use crate::testutils::call_module_cbor;
    use many_identity::testing::identity;
    use many_types::ledger::{TokenInfo, TokenInfoSummary, TokenInfoSupply};
    use mockall::predicate::eq;
    use std::sync::{Arc, Mutex};

    #[test]
    fn create() {
        let mut mock = MockLedgerTokensModuleBackend::new();
        let summary = TokenInfoSummary {
            name: "Foobar".to_string(),
            ticker: "FBR".to_string(),
            decimals: 9,
        };
        let data = TokenCreateArgs {
            summary: summary.clone(),
            owner: None,
            initial_distribution: None,
            maximum_supply: None,
            extended_info: None,
            memo: None,
        };
        let info = TokenInfo {
            symbol: Default::default(),
            summary,
            supply: TokenInfoSupply {
                total: Default::default(),
                circulating: Default::default(),
                maximum: None,
            },
            owner: None,
        };
        mock.expect_create()
            .with(eq(identity(1)), eq(data.clone()))
            .times(1)
            .return_const(Ok(TokenCreateReturns { info: info.clone() }));
        let module = super::LedgerTokensModule::new(Arc::new(Mutex::new(mock)));

        let create_returns: TokenCreateReturns = minicbor::decode(
            &call_module_cbor(1, &module, "tokens.create", minicbor::to_vec(data).unwrap())
                .unwrap(),
        )
        .unwrap();

        assert_eq!(create_returns.info, info);
    }

    #[test]
    fn update() {
        let mut mock = MockLedgerTokensModuleBackend::new();
        let data = TokenUpdateArgs {
            symbol: Default::default(),
            name: None,
            ticker: None,
            decimals: None,
            owner: None,
            memo: None,
        };
        mock.expect_update()
            .with(eq(identity(1)), eq(data.clone()))
            .times(1)
            .returning(|_, _| Ok(TokenUpdateReturns {}));
        let module = super::LedgerTokensModule::new(Arc::new(Mutex::new(mock)));

        let update_returns: TokenUpdateReturns = minicbor::decode(
            &call_module_cbor(1, &module, "tokens.update", minicbor::to_vec(data).unwrap())
                .unwrap(),
        )
        .unwrap();

        assert_eq!(update_returns, TokenUpdateReturns {});
    }

    #[test]
    fn add_extended_info() {
        let mut mock = MockLedgerTokensModuleBackend::new();
        let extended_info = TokenExtendedInfo::new()
            .try_with_memo("Foobar".to_string())
            .unwrap();
        let data = TokenAddExtendedInfoArgs {
            symbol: Default::default(),
            extended_info,
            memo: None,
        };
        mock.expect_add_extended_info()
            .with(eq(identity(1)), eq(data.clone()))
            .times(1)
            .returning(|_, _| Ok(TokenAddExtendedInfoReturns {}));
        let module = super::LedgerTokensModule::new(Arc::new(Mutex::new(mock)));

        let add_ext_info_returns: TokenAddExtendedInfoReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "tokens.addExtendedInfo",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(add_ext_info_returns, TokenAddExtendedInfoReturns {});
    }

    #[test]
    fn remove_extended_info() {
        let mut mock = MockLedgerTokensModuleBackend::new();
        let data = TokenRemoveExtendedInfoArgs {
            symbol: Default::default(),
            extended_info: vec![AttributeRelatedIndex::new(11)],
            memo: None,
        };
        mock.expect_remove_extended_info()
            .with(eq(identity(1)), eq(data.clone()))
            .times(1)
            .returning(|_, _| Ok(TokenRemoveExtendedInfoReturns {}));
        let module = super::LedgerTokensModule::new(Arc::new(Mutex::new(mock)));

        let rm_ext_info_returns: TokenRemoveExtendedInfoReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "tokens.removeExtendedInfo",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(rm_ext_info_returns, TokenRemoveExtendedInfoReturns {});
    }
}
