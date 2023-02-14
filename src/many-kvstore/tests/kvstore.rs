pub mod common;

use crate::common::{setup, Setup};
use many_identity::testing::identity;
use many_identity::Address;
use many_kvstore::error;
use many_modules::kvstore::{
    InfoArg, KvStoreModuleBackend, KvStoreTransferModuleBackend, TransferArgs,
};
use many_types::Either;
use minicbor::bytes::ByteVec;

#[test]
fn info() {
    let Setup {
        module_impl, id, ..
    } = setup();
    let info = module_impl.info(&id, InfoArg {});
    assert!(info.is_ok());
}

#[test]
fn put_get_disable() {
    let mut setup = setup();
    let id = setup.id;
    let put = setup.put(&id, vec![1], vec![2], None);
    assert!(put.is_ok());

    let get_value = setup.get(&id, vec![1]).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let disable = setup.disable(&id, vec![1], None, None);
    assert!(disable.is_ok());

    let get_value = setup.get(&id, vec![1]);
    assert!(get_value.is_err());
    assert_eq!(get_value.unwrap_err().code(), error::key_disabled().code());
}

#[test]
fn put_get_disable_block() {
    let mut setup = Setup::new(true);
    let id = setup.id;
    let (_, put) = setup.block(|setup| setup.put(&id, vec![1], vec![2], None));
    assert!(put.is_ok());

    let get_value = setup.get(&id, vec![1]).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let (_, disable) = setup.block(|setup| setup.disable(&id, vec![1], None, None));
    assert!(disable.is_ok());

    let get_value = setup.get(&id, vec![1]);
    assert!(get_value.is_err());
    assert_eq!(get_value.unwrap_err().code(), error::key_disabled().code());
}

#[test]
fn put_put() {
    let mut setup = setup();
    let id = setup.id;
    let put = setup.put(&id, vec![1], vec![2], None);
    assert!(put.is_ok());

    let get_value = setup.get(&id, vec![1]).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let put = setup.put(&id, vec![1], vec![3], None);
    assert!(put.is_ok());

    let get_value = setup.get(&id, vec![1]).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![3]), get_value);
}

#[test]
fn put_put_unauthorized() {
    let mut setup = setup();
    let id = setup.id;
    let put = setup.put(&id, vec![1], vec![2], None);
    assert!(put.is_ok());

    let get_value = setup.get(&id, vec![1]).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let put = setup.put(&identity(1), vec![1], vec![3], None);
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::permission_denied().code());
}

#[test]
fn put_disable_unauthorized() {
    let mut setup = setup();
    let id = setup.id;
    let put = setup.put(&id, vec![1], vec![2], None);
    assert!(put.is_ok());

    let get_value = setup.get(&id, vec![1]).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let disable = setup.disable(&identity(1), vec![1], None, None);
    assert!(disable.is_err());
    assert_eq!(
        disable.unwrap_err().code(),
        error::permission_denied().code()
    );
}

#[test]
fn put_disable_put() {
    let mut setup = setup();
    let id = setup.id;
    let put = setup.put(&id, vec![1], vec![2], None);
    assert!(put.is_ok());

    let get_value = setup.get(&id, vec![1]).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    let disable = setup.disable(&id, vec![1], None, None);
    assert!(disable.is_ok());

    let get_value = setup.get(&id, vec![1]);
    assert!(get_value.is_err());
    assert_eq!(get_value.unwrap_err().code(), error::key_disabled().code());

    let put = setup.put(&identity(1), vec![1], vec![3], None);
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::permission_denied().code());

    let put = setup.put(&id, vec![1], vec![3], None);
    assert!(put.is_ok());

    let get_value = setup.get(&id, vec![1]).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![3]), get_value);
}

#[test]
fn query() {
    let mut setup = setup();
    let id = setup.id;
    let put = setup.put(&id, vec![1], vec![2], None);
    assert!(put.is_ok());

    let query = setup.query(&id, vec![1]);
    assert!(query.is_ok());

    let query_value = query.unwrap();
    assert_eq!(query_value.disabled, Some(Either::Left(false)));
    assert_eq!(query_value.owner, id);
}

#[test]
fn query_disabled() {
    let mut setup = setup();
    let id = setup.id;
    let put = setup.put(&id, vec![1], vec![2], None);
    assert!(put.is_ok());

    let disable = setup.disable(&id, vec![1], None, None);
    assert!(disable.is_ok());

    let query = setup.query(&id, vec![1]);
    assert!(query.is_ok());

    let query_value = query.unwrap();
    assert_eq!(query_value.disabled, Some(Either::Left(true)));
    assert_eq!(query_value.owner, id);
}

#[test]
fn put_put_illegal() {
    let mut setup = setup();
    let id = setup.id;
    let put = setup.put(&id, vec![1], vec![2], None);
    assert!(put.is_ok());

    let get_value = setup.get(&id, vec![1]).unwrap().value.unwrap();
    assert_eq!(ByteVec::from(vec![2]), get_value);

    setup
        .module_impl
        .transfer(
            &id,
            TransferArgs {
                key: vec![1].into(),
                alternative_owner: None,
                new_owner: Address::illegal(),
            },
        )
        .unwrap();

    let put = setup.put(&identity(1), vec![1], vec![3], None);
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::permission_denied().code());

    let put = setup.put(&identity(1), vec![1], vec![3], Some(Address::illegal()));
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::permission_denied().code());
}
