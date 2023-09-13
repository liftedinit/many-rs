use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;

pub mod info;
pub mod list;

pub use info::*;
pub use list::*;

#[cfg(test)]
use mockall::{automock, predicate::*};

#[many_module(name = WebModule, id = 16, namespace = web, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait WebModuleBackend: Send {
    fn info(&self, sender: &Address, args: InfoArg) -> Result<InfoReturns, ManyError>;

    fn list(&self, sender: &Address, args: ListArgs) -> Result<ListReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use crate::testutils::call_module_cbor;
    use crate::web::{InfoReturns, ListReturns, MockWebModuleBackend};
    use std::sync::{Arc, Mutex};

    #[test]
    fn info() {
        let mut mock = MockWebModuleBackend::new();
        mock.expect_info().times(1).returning(|_sender, _args| {
            Ok(InfoReturns {
                hash: vec![1, 2, 3].into(),
            })
        });
        let module = super::WebModule::new(Arc::new(Mutex::new(mock)));

        let info: InfoReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "web.info",
                minicbor::to_vec(super::InfoArg {}).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(info.hash, vec![1, 2, 3].into());
    }

    #[test]
    fn list() {
        let mut mock = MockWebModuleBackend::new();
        mock.expect_list().times(1).returning(|_sender, _args| {
            Ok(ListReturns {
                deployments: vec![],
            })
        });
        let module = super::WebModule::new(Arc::new(Mutex::new(mock)));

        let list: ListReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "web.list",
                minicbor::to_vec(super::ListArgs {
                    count: None,
                    order: None,
                    filter: None,
                })
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(list.deployments, vec![])
    }
}
