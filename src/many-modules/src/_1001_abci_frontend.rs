use crate::EmptyReturn;
use many_error::{define_attribute_many_error, ManyError};
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

pub type StatusReturn = EmptyReturn;

define_attribute_many_error!(
    attribute 1001 => {
        1: pub fn abci_transport_error(details) => "ABCI interface returned an error: {details}.",
    }
);

#[many_module(name = AbciFrontendModule, id = 1001, namespace = abci, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait AbciClientModuleBackend: Send {
    fn status(&self) -> Result<StatusReturn, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::call_module;
    use std::sync::{Arc, Mutex};

    #[test]
    fn status() {
        let mut mock = MockAbciClientModuleBackend::new();
        mock.expect_status()
            .times(1)
            .returning(|| Ok(StatusReturn {}));
        let module = super::AbciFrontendModule::new(Arc::new(Mutex::new(mock)));

        let _: StatusReturn =
            minicbor::decode(&call_module(1, &module, "abci.status", "null").unwrap()).unwrap();
    }
}
