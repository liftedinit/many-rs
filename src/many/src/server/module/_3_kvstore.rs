use crate::{Identity, ManyError};
use many_macros::many_module;

pub mod get;
pub mod info;
pub use get::*;
pub use info::*;

#[many_module(name = KvStoreModule, id = 3, namespace = kvstore, many_crate = crate)]
pub trait KvStoreModuleBackend: Send {
    fn info(&self, sender: &Identity, args: InfoArgs) -> Result<InfoReturns, ManyError>;
    fn get(&self, sender: &Identity, args: GetArgs) -> Result<GetReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::bytes::ByteVec;

    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use crate::server::module::testutils::call_module_cbor;

    struct KvStoreImpl(pub BTreeMap<ByteVec, ByteVec>);
    impl std::default::Default for KvStoreImpl {
        fn default() -> Self {
            Self(BTreeMap::from([(
                ByteVec::from(vec![1, 3, 5, 7]),
                ByteVec::from(vec![1, 2, 3, 4]),
            )]))
        }
    }

    impl super::KvStoreModuleBackend for KvStoreImpl {
        fn info(
            &self,
            _sender: &crate::Identity,
            _args: super::InfoArgs,
        ) -> Result<InfoReturns, ManyError> {
            Ok(InfoReturns {
                hash: ByteVec::from(vec![1u8; 8]),
            })
        }

        fn get(
            &self,
            _sender: &crate::Identity,
            args: super::GetArgs,
        ) -> Result<super::GetReturns, crate::ManyError> {
            Ok(GetReturns {
                value: self.0.get(&args.key).cloned(),
            })
        }
    }

    #[test]
    fn info() {
        let module_impl = Arc::new(Mutex::new(KvStoreImpl::default()));
        let module = super::KvStoreModule::new(module_impl);

        let data = InfoArgs;
        let data = minicbor::to_vec(data).unwrap();
        let info_returns: InfoReturns =
            minicbor::decode(&call_module_cbor(&module, "kvstore.info", data).unwrap()).unwrap();

        assert_eq!(info_returns.hash, ByteVec::from(vec![1u8; 8]));
    }

    #[test]
    fn get() {
        let module_impl = Arc::new(Mutex::new(KvStoreImpl::default()));
        let module = super::KvStoreModule::new(module_impl);

        let data = GetArgs {
            key: ByteVec::from(vec![1, 3, 5, 7])
        };
        let data = minicbor::to_vec(data).unwrap();

        let get_returns: GetReturns =
            minicbor::decode(&call_module_cbor(&module, "kvstore.get", data).unwrap()).unwrap();

        assert_eq!(get_returns.value, Some(ByteVec::from(vec![1, 2, 3, 4])));
    }
}
