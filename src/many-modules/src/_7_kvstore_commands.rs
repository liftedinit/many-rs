use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

mod disable;
mod put;
pub use disable::*;
pub use put::*;

#[many_module(name = KvStoreCommandsModule, id = 7, namespace = kvstore, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait KvStoreCommandsModuleBackend: Send {
    #[many(deny_anonymous)]
    fn put(&mut self, sender: &Address, args: PutArgs) -> Result<PutReturn, ManyError>;

    #[many(deny_anonymous)]
    fn disable(&mut self, sender: &Address, args: DisableArgs) -> Result<DisableReturn, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::call_module_cbor;
    use many_identity::testing::identity;
    use minicbor::bytes::ByteVec;
    use mockall::predicate;
    use std::sync::{Arc, Mutex};

    #[test]
    fn put() {
        let data = PutArgs {
            key: ByteVec::from(vec![1]),
            value: ByteVec::from(vec![2]),
            alternative_owner: None,
        };

        let mut mock = MockKvStoreCommandsModuleBackend::new();
        mock.expect_put()
            .with(predicate::eq(identity(1)), predicate::eq(data.clone()))
            .times(1)
            .returning(|_sender, _args| Ok(PutReturn {}));
        let module = super::KvStoreCommandsModule::new(Arc::new(Mutex::new(mock)));

        let _: PutReturn = minicbor::decode(
            &call_module_cbor(1, &module, "kvstore.put", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn disable() {
        let data = DisableArgs {
            key: ByteVec::from(vec![1]),
            alternative_owner: None,
            reason: None,
        };

        let mut mock = MockKvStoreCommandsModuleBackend::new();
        mock.expect_disable()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .returning(|_sender, _args| Ok(DisableReturn {}));
        let module = super::KvStoreCommandsModule::new(Arc::new(Mutex::new(mock)));

        let _: DisableReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "kvstore.disable",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }
}
