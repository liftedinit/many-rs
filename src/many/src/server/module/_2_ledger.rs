use crate::{define_attribute_many_error, Identity, ManyError};
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

mod balance;
mod info;

pub use balance::*;
pub use info::*;

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

#[many_module(name = LedgerModule, id = 2, namespace = ledger, many_crate = crate)]
#[cfg_attr(test, automock)]
pub trait LedgerModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, ManyError>;
    fn balance(&self, sender: &Identity, args: BalanceArgs) -> Result<BalanceReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        str::FromStr,
        sync::{Arc, Mutex},
    };

    use minicbor::bytes::ByteVec;
    use once_cell::sync::Lazy;

    use crate::{
        server::module::testutils::{call_module, call_module_cbor},
        types::{ledger::TokenAmount, VecOrSingle},
    };

    use super::*;

    static SYMBOL: Lazy<Identity> = Lazy::new(|| {
        Identity::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap()
    });
    const SYMBOL_NAME: &str = "FOOBAR";

    #[test]
    fn info() {
        let mut mock = MockLedgerModuleBackend::new();
        mock.expect_info().times(1).returning(|_id, _args| {
            Ok(InfoReturns {
                symbols: vec![*SYMBOL],
                hash: ByteVec::from(vec![10u8; 8]),
                local_names: BTreeMap::from([(*SYMBOL, SYMBOL_NAME.to_string())]),
            })
        });
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
        let mut mock = MockLedgerModuleBackend::new();

        mock.expect_balance().times(1).returning(|_id, args| {
            Ok(BalanceReturns {
                balances: BTreeMap::from([(args.symbols.unwrap().0[0], TokenAmount::from(123u16))]),
            })
        });
        let module = super::LedgerModule::new(Arc::new(Mutex::new(mock)));

        let data = BalanceArgs {
            account: None,
            symbols: Some(VecOrSingle::from(vec![*SYMBOL])),
        };
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
