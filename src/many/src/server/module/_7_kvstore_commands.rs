use crate::{Identity, ManyError};
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

mod delete;
mod put;
pub use delete::*;
pub use put::*;

#[many_module(name = KvStoreCommandsModule, id = 7, namespace = kvstore, many_crate = crate)]
#[cfg_attr(test, automock)]
pub trait KvStoreCommandsModuleBackend: Send {
    fn put(&mut self, sender: &Identity, args: PutArgs) -> Result<PutReturn, ManyError>;
    fn delete(&mut self, sender: &Identity, args: DeleteArgs) -> Result<DeleteReturn, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::module::testutils::call_module_cbor;
    use crate::types::identity::tests::identity;
    use minicbor::bytes::ByteVec;
    use mockall::predicate;
    use std::sync::{Arc, Mutex};

    #[test]
    fn put() {
        let data = PutArgs {
            key: ByteVec::from(vec![1]),
            value: ByteVec::from(vec![2]),
        };

        let mut mock = MockKvStoreCommandsModuleBackend::new();
        mock.expect_put()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .returning(|_sender, _args| Ok(PutReturn {}));
        let module = super::KvStoreCommandsModule::new(Arc::new(Mutex::new(mock)));

        let _: PutReturn = minicbor::decode(
            &call_module_cbor(1, &module, "kvstore.put", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn delete() {
        let data = DeleteArgs {
            key: ByteVec::from(vec![1]),
        };

        let mut mock = MockKvStoreCommandsModuleBackend::new();
        mock.expect_delete()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .returning(|_sender, _args| Ok(DeleteReturn {}));
        let module = super::KvStoreCommandsModule::new(Arc::new(Mutex::new(mock)));

        let _: DeleteReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "kvstore.delete",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }
}
