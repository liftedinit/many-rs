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
    use proptest::prelude::*;

    use std::{
        collections::BTreeMap,
        str::FromStr,
        sync::{Arc, Mutex},
    };

    use crate::{
        message::RequestMessage,
        message::{RequestMessageBuilder, ResponseMessage},
        server::tests::execute_request,
        types::{
            identity::{cose::tests::generate_random_eddsa_identity, CoseKeyIdentity},
            ledger::TokenAmount,
        },
        ManyServer,
    };

    const SERVER_VERSION: u8 = 1;
    static SYMBOL: Lazy<Identity> = Lazy::new(|| {
        Identity::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap()
    });
    const SYMBOL_NAME: &str = "FOOBAR";

    #[derive(Default)]
    struct LedgerImpl;

    impl super::LedgerModuleBackend for LedgerImpl {
        // TODO: Fix mock
        fn info(&self, _sender: &Identity, _args: InfoArgs) -> Result<InfoReturns, ManyError> {
            Ok(InfoReturns {
                symbols: vec![*SYMBOL],
                hash: ByteVec::from(vec![1u8; 8]),
                local_names: BTreeMap::from([(*SYMBOL, SYMBOL_NAME.to_string())]),
            })
        }

        // TODO: Fix mock
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

    prop_compose! {
        fn arb_server()(name in "\\PC*") -> (CoseKeyIdentity, Arc<Mutex<ManyServer>>) {
            let id = generate_random_eddsa_identity();
            let server = ManyServer::new(name, id.clone());
            let ledger_impl = Arc::new(Mutex::new(LedgerImpl::default()));
            let ledger_module = LedgerModule::new(ledger_impl);

            {
                let mut s = server.lock().unwrap();
                s.version = Some(SERVER_VERSION.to_string());
                s.add_module(ledger_module);
            }

            (id, server)
        }
    }

    proptest! {
        #[test]
        fn info((id, server) in arb_server()) {
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("ledger.info".to_string())
                .data("null".as_bytes().to_vec())
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let info_returns: InfoReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(info_returns.symbols[0], *SYMBOL);
            assert_eq!(info_returns.hash, ByteVec::from(vec![1u8; 8]));
            assert_eq!(info_returns.local_names.get(&*SYMBOL).unwrap(), &SYMBOL_NAME.to_string());
        }

        #[test]
        fn balance((id, server) in arb_server()) {
            let data = BTreeMap::from([(0, id.identity), (1, *SYMBOL)]);
            let data = minicbor::to_vec(data).unwrap();
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("ledger.balance".to_string())
                .data(data)
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let balance_returns: BalanceReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(balance_returns.balances, BTreeMap::from([(*SYMBOL, TokenAmount::zero())]));
        }
    }
}
