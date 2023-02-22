use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

mod send;

pub use send::*;

#[many_module(name = LedgerCommandsModule, id = 6, namespace = ledger, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait LedgerCommandsModuleBackend: Send {
    fn send(&mut self, sender: &Address, args: SendArgs) -> Result<SendReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::call_module_cbor;
    use many_identity::testing::identity;
    use many_identity::Address;
    use many_types::ledger::TokenAmount;
    use mockall::predicate;
    use std::{
        str::FromStr,
        sync::{Arc, Mutex},
    };

    #[test]
    fn send() {
        let data = SendArgs {
            from: Some(Address::anonymous()),
            to: Address::anonymous(),
            amount: TokenAmount::from(512u16),
            symbol: Address::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz")
                .unwrap(),
            memo: None,
        };
        let mut mock = MockLedgerCommandsModuleBackend::new();
        mock.expect_send()
            .with(predicate::eq(identity(1)), predicate::eq(data.clone()))
            .times(1)
            .returning(|_, _| Ok(SendReturns {}));
        let module = super::LedgerCommandsModule::new(Arc::new(Mutex::new(mock)));

        let _: SendReturns = minicbor::decode(
            &call_module_cbor(1, &module, "ledger.send", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();
    }
}
