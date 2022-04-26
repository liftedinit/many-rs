use crate::{define_attribute_many_error, Identity, ManyError};
use many_macros::many_module;

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
pub trait LedgerModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, ManyError>;
    fn balance(&self, sender: &Identity, args: BalanceArgs) -> Result<BalanceReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::bytes::ByteVec;
    use once_cell::sync::Lazy;

    use std::{
        collections::BTreeMap,
        str::FromStr,
        sync::{Arc, Mutex},
    };

    use crate::{
        server::module::testutils::{call_module_cbor, call_module_cbor_diag},
        types::{identity::cose::tests::generate_random_eddsa_identity, ledger::TokenAmount},
    };

    pub static SYMBOL: Lazy<Identity> = Lazy::new(|| {
        Identity::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap()
    });
    const SYMBOL_NAME: &str = "FOOBAR";

    #[derive(Default)]
    struct LedgerImpl;

    impl super::LedgerModuleBackend for LedgerImpl {
        fn info(&self, _sender: &Identity, _args: InfoArgs) -> Result<InfoReturns, ManyError> {
            Ok(InfoReturns {
                symbols: vec![*SYMBOL],
                hash: ByteVec::from(vec![1u8; 8]),
                local_names: BTreeMap::from([(*SYMBOL, SYMBOL_NAME.to_string())]),
            })
        }

        fn balance(
            &self,
            _sender: &Identity,
            _args: BalanceArgs,
        ) -> Result<BalanceReturns, ManyError> {
            Ok(BalanceReturns {
                balances: BTreeMap::from([(*SYMBOL, TokenAmount::zero())]),
            })
        }
    }

    #[test]
    fn info() {
        let module_impl = Arc::new(Mutex::new(LedgerImpl::default()));
        let module = super::LedgerModule::new(module_impl);

        let info_returns: InfoReturns =
            minicbor::decode(&call_module_cbor_diag(&module, "ledger.info", "null").unwrap())
                .unwrap();

        assert_eq!(info_returns.symbols[0], *SYMBOL);
        assert_eq!(info_returns.hash, ByteVec::from(vec![1u8; 8]));
        assert_eq!(
            info_returns.local_names.get(&*SYMBOL).unwrap(),
            &SYMBOL_NAME.to_string()
        );
    }

    #[test]
    fn balance() {
        let module_impl = Arc::new(Mutex::new(LedgerImpl::default()));
        let module = super::LedgerModule::new(module_impl);
        let id = generate_random_eddsa_identity();

        let data = BTreeMap::from([(0, id.identity), (1, *SYMBOL)]);
        let data = minicbor::to_vec(data).unwrap();
        let balance_returns: BalanceReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.balance", data).unwrap()).unwrap();

        assert_eq!(
            balance_returns.balances,
            BTreeMap::from([(*SYMBOL, TokenAmount::zero())])
        );
    }
}
