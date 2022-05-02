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
    use std::sync::{Arc, Mutex};

    use minicbor::bytes::ByteVec;

    use crate::server::module::testutils::call_module_cbor;

    use super::*;

    #[test]
    fn put() {
        let mut mock = MockKvStoreCommandsModuleBackend::new();
        mock.expect_put()
            .times(1)
            .returning(|_sender, _args| Ok(PutReturn {} ));
        let module = super::KvStoreCommandsModule::new(Arc::new(Mutex::new(mock)));

        let data = PutArgs {
            key: ByteVec::from(vec![1]),
            value: ByteVec::from(vec![2]),
        };

        let _: PutReturn = minicbor::decode(
            &call_module_cbor(1, &module, "kvstore.put", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn delete() {
        let mut mock = MockKvStoreCommandsModuleBackend::new();
        mock.expect_delete()
            .times(1)
            .returning(|_sender, _args| Ok(DeleteReturn {} ));
        let module = super::KvStoreCommandsModule::new(Arc::new(Mutex::new(mock)));

        let data = DeleteArgs {
            key: ByteVec::from(vec![1]),
        };

        let _: DeleteReturn = minicbor::decode(
            &call_module_cbor(1, &module, "kvstore.delete", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();
    }
}