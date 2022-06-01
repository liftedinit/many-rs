use crate::{Identity, ManyError};
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

pub mod errors;
mod get;
mod store;
pub mod types;

pub use errors::*;
pub use get::*;
pub use store::*;
pub use types::*;

#[many_module(name = IdStoreModule, id = 1002, namespace = idstore, many_crate = crate, drop_non_webauthn = [store])]
#[cfg_attr(test, automock)]
pub trait IdStoreModuleBackend: Send {
    fn store(&mut self, sender: &Identity, args: StoreArgs) -> Result<StoreReturns, ManyError>;
    fn get_from_recall_phrase(
        &self,
        args: GetFromRecallPhraseArgs,
    ) -> Result<GetReturns, ManyError>;
    fn get_from_address(&self, args: GetFromAddressArgs) -> Result<GetReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        server::module::testutils::call_module_cbor,
        types::identity::{cose::testsutils::generate_random_eddsa_identity, testing::identity},
        Identity,
    };
    use coset::CborSerializable;
    use minicbor::bytes::ByteVec;
    use mockall::predicate;
    use std::{
        str::FromStr,
        sync::{Arc, Mutex},
    };

    #[test]
    fn store() {
        let id = generate_random_eddsa_identity();
        let data = StoreArgs {
            address: id.identity,
            cred_id: CredentialId(ByteVec::from(Vec::from([1u8; 16]))),
            public_key: PublicKey(ByteVec::from(id.key.unwrap().to_vec().unwrap())),
        };
        let ret = StoreReturns(vec!["foo".to_string(), "bar".to_string()]);
        let mut mock: MockIdStoreModuleBackend = MockIdStoreModuleBackend::new();
        mock.expect_store()
            .with(
                predicate::eq(tests::identity(1)),
                predicate::eq(data.clone()),
            )
            .times(1)
            .return_const(Ok(ret.clone()));

        let module = super::IdStoreModule::new(Arc::new(Mutex::new(mock)));
        let store_returns: StoreReturns = minicbor::decode(
            &call_module_cbor(1, &module, "idstore.store", minicbor::to_vec(data).unwrap())
                .unwrap(),
        )
        .unwrap();

        assert_eq!(store_returns.0, ret.0);
    }

    #[test]
    fn get_from_recall_phrase() {
        let id = generate_random_eddsa_identity();
        let data = GetFromRecallPhraseArgs(vec!["foo".to_string(), "bar".to_string()]);
        let ret = GetReturns {
            cred_id: CredentialId(ByteVec::from(Vec::from([1u8; 16]))),
            public_key: PublicKey(ByteVec::from(id.key.unwrap().to_vec().unwrap())),
        };
        let mut mock: MockIdStoreModuleBackend = MockIdStoreModuleBackend::new();
        mock.expect_get_from_recall_phrase()
            .with(predicate::eq(data.clone()))
            .times(1)
            .return_const(Ok(ret.clone()));

        let module = super::IdStoreModule::new(Arc::new(Mutex::new(mock)));
        let get_returns: GetReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "idstore.getFromRecallPhrase",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(get_returns.cred_id, ret.cred_id);
        assert_eq!(get_returns.public_key, ret.public_key);
    }

    #[test]
    fn get_from_address() {
        let id = generate_random_eddsa_identity();
        let data = GetFromAddressArgs(
            Identity::from_str("maffbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wijp").unwrap(),
        );
        let ret = GetReturns {
            cred_id: CredentialId(ByteVec::from(Vec::from([1u8; 16]))),
            public_key: PublicKey(ByteVec::from(id.key.unwrap().to_vec().unwrap())),
        };
        let mut mock: MockIdStoreModuleBackend = MockIdStoreModuleBackend::new();
        mock.expect_get_from_address()
            .with(predicate::eq(data.clone()))
            .times(1)
            .return_const(Ok(ret.clone()));

        let module = super::IdStoreModule::new(Arc::new(Mutex::new(mock)));
        let get_returns: GetReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "idstore.getFromAddress",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(get_returns.cred_id, ret.cred_id);
        assert_eq!(get_returns.public_key, ret.public_key);
    }
}
