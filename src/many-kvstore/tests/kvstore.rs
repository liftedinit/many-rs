pub mod common;

use crate::common::{setup, Setup};
use many_error::Reason;
use many_identity::testing::identity;
use many_identity::Address;
use many_kvstore::error;
use many_modules::kvstore::{
    InfoArg, KvStoreModuleBackend, KvStoreTransferModuleBackend, TransferArgs,
};
use many_types::{Either, SortOrder};
use minicbor::bytes::ByteVec;
use std::collections::BTreeMap;

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

#[test]
fn list() {
    let mut setup = setup();
    let id = setup.id;
    let keys = vec![vec![1], vec![2], vec![3], vec![4], vec![5]];
    for k in &keys {
        let put = setup.put(&id, k.clone(), vec![1], None);
        assert!(put.is_ok());
    }

    let list = setup.list(&setup.id, SortOrder::Ascending).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys
    );

    let list = setup.list(&setup.id, SortOrder::Descending).unwrap().keys;
    assert_eq!(
        list.into_iter()
            .rev()
            .map(|e| e.into())
            .collect::<Vec<Vec<u8>>>(),
        keys
    );

    // Disable first key with bool
    let _ = setup.disable(&id, vec![1], None, None).unwrap();
    let query = setup.query(&id, vec![1]).unwrap();
    assert_eq!(query.disabled, Some(Either::Left(true)));

    // Verify that disabled key is not listed
    let list = setup.list(&setup.id, SortOrder::Ascending).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys[1..].to_vec()
    );

    // Disable second key with reason
    let reason = Reason::new(123, Some("Foo".to_string()), BTreeMap::new());
    let _ = setup
        .disable(&id, vec![2], None, Some(reason.clone()))
        .unwrap();
    let query = setup.query(&id, vec![2]).unwrap();
    assert_eq!(query.disabled, Some(Either::Right(reason)));

    // Verify that disabled key is not listed
    let list = setup.list(&setup.id, SortOrder::Ascending).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys[2..].to_vec()
    );

    // Re-enable first key
    let _ = setup.put(&id, vec![2], b"foo".to_vec(), None);

    // Verify re-enabled key is listed
    let list = setup.list(&setup.id, SortOrder::Ascending).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys[1..].to_vec()
    );
}
