use crate::EmptyReturn;
use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;
use many_types::{cbor_type_decl, ledger, Memo};
use minicbor::{Decode, Encode};

cbor_type_decl!(
    pub struct TokenMintArgs {
        0 => symbol: ledger::Symbol,
        1 => distribution: ledger::LedgerTokensAddressMap,
        2 => memo: Option<Memo>,
    }

    pub struct TokenBurnArgs {
        0 => symbol: ledger::Symbol,
        1 => distribution: ledger::LedgerTokensAddressMap,
        2 => memo: Option<Memo>,
        3 => error_on_under_burn: Option<bool>,
    }

    pub struct TokenBurnReturns {
        0 => distribution: ledger::LedgerTokensAddressMap,
    }
);

pub type TokenMintReturns = EmptyReturn;

#[many_module(name = LedgerMintBurnModule, id = 12, namespace = tokens, many_modules_crate = crate)]
#[cfg_attr(test, mockall::automock)]
pub trait LedgerMintBurnModuleBackend: Send {
    fn mint(
        &mut self,
        sender: &Address,
        args: TokenMintArgs,
    ) -> Result<TokenMintReturns, ManyError>;
    fn burn(
        &mut self,
        sender: &Address,
        args: TokenBurnArgs,
    ) -> Result<TokenBurnReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::call_module_cbor;
    use many_identity::testing::identity;
    use mockall::predicate::eq;
    use std::sync::{Arc, Mutex};

    #[test]
    fn mint() {
        let mut mock = MockLedgerMintBurnModuleBackend::new();
        let data = TokenMintArgs {
            symbol: Default::default(),
            distribution: Default::default(),
            memo: None,
        };
        mock.expect_mint()
            .with(eq(identity(1)), eq(data.clone()))
            .times(1)
            .returning(|_, _| Ok(TokenMintReturns {}));
        let module = super::LedgerMintBurnModule::new(Arc::new(Mutex::new(mock)));

        let update_returns: TokenMintReturns = minicbor::decode(
            &call_module_cbor(1, &module, "tokens.mint", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();

        assert_eq!(update_returns, TokenMintReturns {});
    }

    #[test]
    fn burn() {
        let mut mock = MockLedgerMintBurnModuleBackend::new();
        let data = TokenBurnArgs {
            symbol: Default::default(),
            distribution: Default::default(),
            memo: None,
            error_on_under_burn: None,
        };
        mock.expect_burn()
            .with(eq(identity(1)), eq(data.clone()))
            .times(1)
            .return_const(Ok(TokenBurnReturns {
                distribution: Default::default(),
            }));
        let module = super::LedgerMintBurnModule::new(Arc::new(Mutex::new(mock)));

        let update_returns: TokenBurnReturns = minicbor::decode(
            &call_module_cbor(1, &module, "tokens.burn", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();

        assert_eq!(
            update_returns,
            TokenBurnReturns {
                distribution: Default::default()
            }
        );
    }
}
