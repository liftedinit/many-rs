use crate::{Identity, ManyError};
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

pub mod get;
pub mod info;
pub use get::*;
pub use info::*;

#[many_module(name = KvStoreModule, id = 3, namespace = kvstore, many_crate = crate)]
#[cfg_attr(test, automock)]
pub trait KvStoreModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, ManyError>;
    fn get(&self, sender: &Identity, args: GetArgs) -> Result<GetReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use minicbor::bytes::ByteVec;

    use super::*;

    use crate::server::module::testutils::{call_module, call_module_cbor};
    use std::sync::{Arc, Mutex};

    #[test]
    fn info() {
        let mut mock = MockKvStoreModuleBackend::new();
        mock.expect_info().times(1).returning(|_id, _args| {
            Ok(InfoReturns {
                hash: ByteVec::from(vec![9u8; 8]),
            })
        });
        let module = super::KvStoreModule::new(Arc::new(Mutex::new(mock)));
        let info_returns: InfoReturns =
            minicbor::decode(&call_module(1, &module, "kvstore.info", "null").unwrap()).unwrap();

        assert_eq!(info_returns.hash, ByteVec::from(vec![9u8; 8]));
    }

    #[test]
    fn get() {
        let mut mock = MockKvStoreModuleBackend::new();
        mock.expect_get().times(1).returning(|_id, _args| {
            Ok(GetReturns {
                value: Some(ByteVec::from(vec![1, 2, 3, 4])),
            })
        });
        let module = super::KvStoreModule::new(Arc::new(Mutex::new(mock)));

        let data = GetArgs {
            key: ByteVec::from(vec![5, 6, 7]),
        };
        let get_returns: GetReturns = minicbor::decode(
            &call_module_cbor(1, &module, "kvstore.get", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();

        assert_eq!(get_returns.value, Some(ByteVec::from(vec![1, 2, 3, 4])));
    }
}
