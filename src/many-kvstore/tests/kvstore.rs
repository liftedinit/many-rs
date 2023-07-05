pub mod common;

use std::collections::BTreeMap;
use crate::common::{setup, Setup};
use many_identity::testing::identity;
use many_identity::Address;
use many_kvstore::error;
use many_modules::kvstore::{InfoArg, KeyFilterType, KvStoreModuleBackend, KvStoreTransferModuleBackend, TransferArgs};
use many_types::{Either, SortOrder};
use minicbor::bytes::ByteVec;
use many_error::Reason;

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
fn list_order() {
    let mut setup = setup();
    let id = setup.id;
    let keys = vec![vec![1], vec![2], vec![3], vec![4], vec![5]];
    for k in &keys {
        let put = setup.put(&id, k.clone(), vec![1], None);
        assert!(put.is_ok());
    }

    // List all keys, ascending order
    let list = setup.list(&setup.id, SortOrder::Ascending, None).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys
    );

    // List all keys, descending order
    let list = setup.list(&setup.id, SortOrder::Descending, None).unwrap().keys;
    assert_eq!(
        list.into_iter()
            .rev()
            .map(|e| e.into())
            .collect::<Vec<Vec<u8>>>(),
        keys
    );
}

#[test]
fn list_filter_with_owner() {
    let mut setup = setup();
    let id = setup.id;
    let keys = vec![vec![1u8], vec![2], vec![3], vec![4], vec![5]];
    let keys2 = vec![vec![11u8], vec![22], vec![33], vec![44], vec![55]];
    for k in 0..5 {
        let put = setup.put(&id, keys[k].clone(), vec![1], None);
        assert!(put.is_ok());
        let put2 = setup.put(&identity(666), keys2[k].clone(), vec![1], None);
        assert!(put2.is_ok());
    }

    // List only keys belonging to id
    let list = setup.list(&setup.id, SortOrder::Ascending, Some(vec![KeyFilterType::Owner(id)])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys
    );

    // List only keys belonging to identity(666)
    let list = setup.list(&identity(666), SortOrder::Ascending, Some(vec![KeyFilterType::Owner(identity(666))])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys2
    );
}

#[test]
fn list_filter_with_owner_and_disabled() {
    let mut setup = setup();
    let id = setup.id;
    let keys = vec![vec![1u8], vec![2], vec![3], vec![4], vec![5]];
    let keys2 = vec![vec![11u8], vec![22], vec![33], vec![44], vec![55]];
    for k in 0..5 {
        let put = setup.put(&id, keys[k].clone(), vec![1], None);
        assert!(put.is_ok());
        let put2 = setup.put(&identity(666), keys2[k].clone(), vec![1], None);
        assert!(put2.is_ok());
    }

    // Disable first key belonging to id
    let _ = setup.disable(&id, vec![1], None, None).unwrap();
    let query = setup.query(&id, vec![1]).unwrap();
    assert_eq!(query.disabled, Some(Either::Left(true)));

    // List keys belonging to id that are disabled
    let list = setup.list(&setup.id, SortOrder::Ascending, Some(vec![KeyFilterType::Owner(id), KeyFilterType::Disabled(true)])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        vec![keys[0].clone()]
    );

    // List keys belonging to id that are not disabled
    let list = setup.list(&setup.id, SortOrder::Ascending, Some(vec![KeyFilterType::Owner(id), KeyFilterType::Disabled(false)])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys[1..].to_vec()
    );

    // Disable first key belonging to identity(666)
    let reason =Reason::new(123, Some("foo".to_string()), BTreeMap::new());
    let _ = setup.disable(&identity(666), vec![11], None, Some(reason.clone())).unwrap();
    let query = setup.query(&identity(666), vec![11]).unwrap();
    assert_eq!(query.disabled, Some(Either::Right(reason)));

    // List keys belonging to identity(666) that are disabled
    let list = setup.list(&identity(666), SortOrder::Ascending, Some(vec![KeyFilterType::Owner(identity(666)), KeyFilterType::Disabled(true)])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        vec![keys2[0].clone()]
    );

    // List keys belonging to identity(666) that are not disabled
    let list = setup.list(&identity(666), SortOrder::Ascending, Some(vec![KeyFilterType::Owner(identity(666)), KeyFilterType::Disabled(false)])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys2[1..].to_vec()
    );

    // Re-enable first key belonging to id
    let _ = setup.put(&id, vec![1], b"foo".to_vec(), None);

    // Verify re-enabled key is listed when querying for keys owned by id and that are not disabled
    let list = setup.list(&setup.id, SortOrder::Ascending,Some(vec![KeyFilterType::Owner(id), KeyFilterType::Disabled(false)]) ).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys
    );
}

#[test]
fn list_filter_previous_owner() {
    let mut setup = setup();
    let id = setup.id;
    let keys = vec![vec![1u8], vec![2], vec![3], vec![4], vec![5]];
    let keys2 = vec![vec![11u8], vec![22], vec![33], vec![44], vec![55]];
    for k in 0..5 {
        let put = setup.put(&id, keys[k].clone(), vec![1], None);
        assert!(put.is_ok());
        let put2 = setup.put(&identity(666), keys2[k].clone(), vec![1], None);
        assert!(put2.is_ok());
    }

    // Mark first key belonging to id as immutable
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

    // List keys belonging to id
    let list = setup.list(&setup.id, SortOrder::Ascending, Some(vec![KeyFilterType::Owner(id)])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        keys[1..].to_vec()
    );

    // List keys belonging to illegal address
    let list = setup.list(&setup.id, SortOrder::Ascending, Some(vec![KeyFilterType::Owner(Address::illegal())])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        vec![keys[0].clone()]
    );

    // Mark first key belonging to identity(666) as immutable
    setup
        .module_impl
        .transfer(
            &identity(666),
            TransferArgs {
                key: vec![11].into(),
                alternative_owner: None,
                new_owner: Address::illegal(),
            },
        )
        .unwrap();

    // List all immutable keys
    let list = setup.list(&setup.id, SortOrder::Ascending, Some(vec![KeyFilterType::Owner(Address::illegal())])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        vec![keys[0].clone(), keys2[0].clone()]
    );

    // List immutable keys where id is the previous owner
    let list = setup.list(&setup.id, SortOrder::Ascending, Some(vec![KeyFilterType::Owner(Address::illegal()), KeyFilterType::PreviousOwner(id)])).unwrap().keys;
    assert_eq!(
        list.into_iter().map(|e| e.into()).collect::<Vec<Vec<u8>>>(),
        vec![keys[0].clone()]
    );
}