pub mod common;

use crate::common::*;
use many_error::Reason;
use many_identity::testing::identity;
use many_identity::Address;
use many_modules::account;
use many_modules::account::Role;
use std::collections::BTreeMap;

use many_kvstore::error;
use many_types::Either;

#[test]
fn put_as_acc() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let id = setup.id();
    let account_id = setup.account_id;

    let put = setup.put(&id, vec![1], vec![2], Some(account_id));
    assert!(put.is_ok());

    let get = setup.get(&Address::anonymous(), vec![1]);
    assert!(get.is_ok());
    assert_eq!(get.unwrap().value.unwrap(), vec![2].into());

    let query = setup.query(&Address::anonymous(), vec![1]);
    assert!(query.is_ok());
    assert_eq!(query.unwrap().owner, account_id);
}

#[test]
fn put_as_alt_invalid_addr() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let id = setup.id();

    let put = setup.put(&id, vec![1], vec![2], Some(identity(666)));
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::permission_denied().code());
}

#[test]
fn put_as_alt_anon() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let id = setup.id();

    let put = setup.put(&id, vec![1], vec![2], Some(Address::anonymous()));
    assert!(put.is_err());
    assert_eq!(put.unwrap_err().code(), error::anon_alt_denied().code());
}

#[test]
fn put_as_alt_subres() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let id = setup.id();

    let put = setup.put(
        &id,
        vec![1],
        vec![2],
        Some(id.with_subresource_id(2).unwrap()),
    );
    assert!(put.is_err());
    assert_eq!(
        put.unwrap_err().code(),
        error::subres_alt_unsupported().code()
    );
}

#[test]
fn put_as_sender_not_in_acc() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let account_id = setup.account_id;

    let put = setup.put(&identity(666), vec![1], vec![2], Some(account_id));
    assert!(put.is_err());
    assert_eq!(
        put.unwrap_err().code(),
        account::errors::user_needs_role(Role::CanKvStorePut).code()
    );
}

#[test]
fn put_as_sender_invalid_role() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let account_id = setup.account_id;

    let put = setup.put(&identity(3), vec![1], vec![2], Some(account_id));
    assert!(put.is_err());
    assert_eq!(
        put.unwrap_err().code(),
        account::errors::user_needs_role(Role::CanKvStorePut).code()
    );
}

#[test]
fn put_as_alt_user_in_acc_with_perm() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let account_id = setup.account_id;

    let put = setup.put(&identity(2), vec![1], vec![2], Some(account_id));
    assert!(put.is_ok());

    let get = setup.get(&Address::anonymous(), vec![1]);
    assert!(get.is_ok());
    assert_eq!(get.unwrap().value.unwrap(), vec![2].into());

    let query = setup.query(&Address::anonymous(), vec![1]);
    assert!(query.is_ok());
    assert_eq!(query.unwrap().owner, account_id);
}

#[test]
fn query_put_as() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let id = setup.id();
    let account_id = setup.account_id;
    let put = setup.put(&id, vec![1], vec![2], Some(account_id));
    assert!(put.is_ok());

    let query = setup.query(&id, vec![1]);
    assert!(query.is_ok());

    let query_value = query.unwrap();
    assert_eq!(query_value.disabled, Some(Either::Left(false)));
    assert_eq!(query_value.owner, account_id);
}

#[test]
fn query_disable_as() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let id = setup.id();
    let account_id = setup.account_id;
    let put = setup.put(&id, vec![1], vec![2], Some(account_id));
    assert!(put.is_ok());

    let put = setup.disable(&id, vec![1], Some(account_id), None);
    assert!(put.is_ok());

    let query = setup.query(&id, vec![1]);
    assert!(query.is_ok());

    let query_value = query.unwrap();
    assert_eq!(query_value.disabled, Some(Either::Left(true)));
    assert_eq!(query_value.owner, account_id);
}

#[test]
fn query_disable_reason_as() {
    let mut setup = setup_with_account(AccountType::KvStore);
    let id = setup.id();
    let account_id = setup.account_id;
    let put = setup.put(&id, vec![1], vec![2], Some(account_id));
    assert!(put.is_ok());

    let reason = Reason::new(12345, Some("Foo".to_string()), BTreeMap::new());
    let put = setup.disable(&id, vec![1], Some(account_id), Some(reason.clone()));
    assert!(put.is_ok());

    let query = setup.query(&id, vec![1]);
    assert!(query.is_ok());

    let query_value = query.unwrap();
    assert_eq!(query_value.disabled, Some(Either::Right(reason)));
    assert_eq!(query_value.owner, account_id);
}
