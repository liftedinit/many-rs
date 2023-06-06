use many_error::{define_attribute_many_error, ManyError};
use many_macros::many_module;
use many_protocol::context::Context;

#[cfg(test)]
use mockall::{automock, predicate::*};

mod balance;
mod info;

pub use balance::*;
pub use info::*;
use many_identity::Address;

define_attribute_many_error!(
    attribute 2 => {
        1: pub fn unknown_symbol(symbol) => "Symbol not supported by this ledger: {symbol}.",
        2: pub fn unauthorized() => "Unauthorized to do this operation.",
        3: pub fn insufficient_funds() => "Insufficient funds.",
        4: pub fn anonymous_cannot_hold_funds() => "Anonymous is not a valid account identity.",
        5: pub fn invalid_initial_state(expected, actual)
            => "Invalid initial state hash. Expected '{expected}', was '{actual}'.",
    }
);

#[many_module(name = LedgerModule, id = 2, namespace = ledger, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait LedgerModuleBackend: Send {
    fn info(
        &self,
        sender: &Address,
        args: InfoArgs,
        context: Context,
    ) -> Result<InfoReturns, ManyError>;
    fn balance(
        &self,
        sender: &Address,
        args: BalanceArgs,
        context: Context,
    ) -> Result<BalanceReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::{call_module, call_module_cbor};
    use many_identity::testing::identity;
    use many_identity::Address;
    use many_types::ledger::TokenAmount;
    use many_types::VecOrSingle;
    use minicbor::bytes::ByteVec;
    use mockall::predicate;
    use once_cell::sync::Lazy;
    use std::{
        collections::BTreeMap,
        str::FromStr,
        sync::{Arc, Mutex},
    };

    static SYMBOL: Lazy<Address> = Lazy::new(|| {
        Address::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap()
    });
    const SYMBOL_NAME: &str = "FOOBAR";

    #[test]
    fn info() {
        let mut mock = MockLedgerModuleBackend::new();
        mock.expect_info()
            .with(
                predicate::eq(identity(1)),
                predicate::eq(InfoArgs {}),
                predicate::always(),
            )
            .times(1)
            .return_const(Ok(InfoReturns {
                symbols: vec![*SYMBOL],
                hash: ByteVec::from(vec![10u8; 8]),
                local_names: BTreeMap::from([(*SYMBOL, SYMBOL_NAME.to_string())]),
                tokens: Default::default(),
            }));
        let module = super::LedgerModule::new(Arc::new(Mutex::new(mock)));

        let info_returns: InfoReturns =
            minicbor::decode(&call_module(1, &module, "ledger.info", "null").unwrap()).unwrap();

        assert_eq!(info_returns.symbols[0], *SYMBOL);
        assert_eq!(info_returns.hash, ByteVec::from(vec![10u8; 8]));
        assert_eq!(
            info_returns.local_names.get(&*SYMBOL).unwrap(),
            &SYMBOL_NAME.to_string()
        );
    }

    #[test]
    fn balance() {
        let data = BalanceArgs {
            account: None,
            symbols: Some(VecOrSingle::from(vec![*SYMBOL])),
        };
        let mut mock = MockLedgerModuleBackend::new();
        mock.expect_balance()
            .with(
                predicate::eq(identity(1)),
                predicate::eq(data.clone()),
                predicate::always(),
            )
            .times(1)
            .returning(|_, args, _| {
                Ok(BalanceReturns {
                    balances: BTreeMap::from([(
                        args.symbols.unwrap().0[0],
                        TokenAmount::from(123u16),
                    )]),
                })
            });
        let module = super::LedgerModule::new(Arc::new(Mutex::new(mock)));

        let balance_returns: BalanceReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "ledger.balance",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            balance_returns.balances,
            BTreeMap::from([(*SYMBOL, TokenAmount::from(123u16))])
        );
    }
}
