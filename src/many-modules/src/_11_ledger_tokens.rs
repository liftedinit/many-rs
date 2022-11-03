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
        1 => owner: Option<Address>,
        2 => initial_distribution: Option<ledger::LedgerTokensAddressMap>,
        3 => maximum_supply: Option<ledger::TokenAmount>,
        4 => extended_info: Option<extended_info::TokenExtendedInfo>,
    }

    pub struct TokenCreateReturns {
        0 => info: ledger::TokenInfo,
    }

    pub struct TokenInfoArgs {
        0 => symbol: ledger::Symbol,
        1 => extended_info: Option<Vec<AttributeRelatedIndex>>,
    }

    pub struct TokenInfoReturns {
        0 => info: ledger::TokenInfo,
        1 => extended_info: extended_info::TokenExtendedInfo,
    }

    pub struct TokenUpdateArgs {
        0 => symbol: ledger::Symbol,
        1 => name: Option<String>,
        2 => ticker: Option<String>,
        3 => decimals: Option<u32>,
        4 => owner: Option<Address>,
        5 => memo: Option<Memo>,
    }

    pub struct TokenAddExtendedInfoArgs {
        0 => symbol: ledger::Symbol,
        1 => extended_info: extended_info::TokenExtendedInfo,
    }

    pub struct TokenRemoveExtendedInfoArgs {
        0 => symbol: ledger::Symbol,
        1 => extended_info: Vec<AttributeRelatedIndex>,
    }
);

pub type TokenUpdateReturns = EmptyReturn;
pub type TokenAddExtendedInfoReturns = EmptyReturn;
pub type TokenRemoveExtendedInfoReturns = EmptyReturn;

#[many_module(name = LedgerTokensModule, id = 11, namespace = ledger, many_modules_crate = crate)]
#[cfg_attr(test, mockall::automock)]
pub trait LedgerTokensModuleBackend: Send {
    fn create(
        &mut self,
        sender: &Address,
        args: TokenCreateArgs,
    ) -> Result<TokenCreateReturns, ManyError>;

    fn info(&self, sender: &Address, args: TokenInfoArgs) -> Result<TokenInfoReturns, ManyError>;

    fn update(
        &mut self,
        sender: &Address,
        args: TokenUpdateArgs,
    ) -> Result<TokenUpdateReturns, ManyError>;

    fn add_extended_info(
        &mut self,
        sender: &Address,
        args: TokenAddExtendedInfoArgs,
    ) -> Result<TokenAddExtendedInfoReturns, ManyError>;

    fn remove_extended_info(
        &mut self,
        sender: &Address,
        args: TokenRemoveExtendedInfoArgs,
    ) -> Result<TokenRemoveExtendedInfoReturns, ManyError>;
}
