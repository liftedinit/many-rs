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
    use proptest::prelude::*;

    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use crate::{
        message::RequestMessage,
        message::{RequestMessageBuilder, ResponseMessage},
        server::tests::execute_request,
        types::identity::{cose::tests::generate_random_eddsa_identity, CoseKeyIdentity},
        ManyServer,
    };

    const SERVER_VERSION: u8 = 1;

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

    prop_compose! {
        fn arb_server()(name in "\\PC*") -> (CoseKeyIdentity, Arc<Mutex<ManyServer>>) {
            let id = generate_random_eddsa_identity();
            let server = ManyServer::new(name, id.clone());
            let kvstore_impl = Arc::new(Mutex::new(KvStoreImpl::default()));
            let kvstore_module = KvStoreModule::new(kvstore_impl);

            {
                let mut s = server.lock().unwrap();
                s.version = Some(SERVER_VERSION.to_string());
                s.add_module(kvstore_module);
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
                .method("kvstore.info".to_string())
                .data("null".as_bytes().to_vec())
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let info_returns: InfoReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(info_returns.hash, ByteVec::from(vec![1u8; 8]));
        }

        #[test]
        fn get((id, server) in arb_server()) {
            let mut data = BTreeMap::new();
            data.insert(0, ByteVec::from(vec![1, 3, 5, 7]));
            let data = minicbor::to_vec(data).unwrap();
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("kvstore.get".to_string())
                .data(data)
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();

            let get_returns: GetReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(get_returns.value, Some(ByteVec::from(vec![1, 2, 3, 4])));
        }
    }
}
