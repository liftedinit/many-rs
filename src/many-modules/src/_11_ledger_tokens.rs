use crate::_6_ledger_commands::{SendArgs, SendReturns};
use crate::ledger::TokenInfoSummary;
use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;
use many_types::ledger;
use minicbor::{Decode, Encode};
use mockall::automock;

pub mod extended_info;

#[derive(Clone, Debug, Decode, Encode)]
#[cbor(map)]
pub struct TokenInfo {}

#[derive(Clone, Debug, Decode, Encode)]
#[cbor(map)]
pub struct TokenCreateArgs {
    #[n(0)]
    pub summary: TokenInfoSummary,

    #[n(1)]
    pub owner: Option<Address>,

    #[n(2)]
    pub initial_distribution: Option<ledger::LedgerTokensAddressMap>,

    #[n(3)]
    pub maximum_supply: Option<ledger::TokenAmount>,

    #[n(4)]
    pub extended_info: Option<extended_info::TokenExtendedInfo>,
}

#[derive(Clone, Debug, Decode, Encode)]
#[cbor(map)]
pub struct TokenCreateReturns {
    #[n(0)]
    pub info: TokenInfo,
}

#[many_module(name = LedgerTokensModule, id = 11, namespace = ledger, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait LedgerTokensModuleBackend: Send {
    fn create(
        &mut self,
        sender: &Address,
        args: TokenCreateArgs,
    ) -> Result<TokenCreateReturns, ManyError>;
}
