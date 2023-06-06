use many_error::ManyError;
use many_identity::Address;
use many_ledger::module::LedgerModuleImpl;
use many_ledger_test_utils::*;
use many_modules::idstore;
use many_modules::idstore::{CredentialId, IdStoreModuleBackend, PublicKey};

pub struct SetupWithArgs {
    pub module_impl: LedgerModuleImpl,
    pub id: Address,
    pub args: idstore::StoreArgs,
}

fn setup_with_args() -> SetupWithArgs {
    let Setup {
        module_impl,
        id,
        cred_id,
        public_key,
        ..
    } = setup();
    SetupWithArgs {
        module_impl,
        id,
        args: idstore::StoreArgs {
            address: id,
            cred_id,
            public_key,
        },
    }
}

pub struct SetupWithStore {
    pub module_impl: LedgerModuleImpl,
    pub id: Address,
    pub cred_id: CredentialId,
    pub public_key: PublicKey,
    pub recall_phrase: Vec<String>,
}

fn setup_with_store() -> SetupWithStore {
    let SetupWithArgs {
        mut module_impl,
        id,
        args,
    } = setup_with_args();
    let result = module_impl.store(&id, args.clone());
    assert!(result.is_ok());
    SetupWithStore {
        module_impl,
        id,
        cred_id: args.cred_id,
        public_key: args.public_key,
        recall_phrase: result.unwrap().0,
    }
}

#[test]
/// Verify basic id storage
fn store() {
    let SetupWithArgs {
        mut module_impl,
        id,
        args,
    } = setup_with_args();
    let result = module_impl.store(&id, args);
    assert!(result.is_ok());
}

#[test]
/// Verify we're unable to store as anonymous
fn store_anon() {
    let SetupWithArgs {
        mut module_impl,
        args,
        ..
    } = setup_with_args();
    let result = module_impl.store(&Address::anonymous(), args);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        ManyError::invalid_identity().code()
    );
}

#[test]
/// Verify we're unable to store when credential ID is too small
fn invalid_cred_id_too_small() {
    let SetupWithArgs {
        mut module_impl,
        id,
        mut args,
    } = setup_with_args();
    args.cred_id = CredentialId(vec![1; 15].into());
    let result = module_impl.store(&id, args);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        idstore::invalid_credential_id("".to_string()).code()
    );
}

#[test]
/// Verify we're unable to store when credential ID is too long
fn invalid_cred_id_too_long() {
    let SetupWithArgs {
        mut module_impl,
        id,
        mut args,
    } = setup_with_args();
    args.cred_id = CredentialId(vec![1; 1024].into());
    let result = module_impl.store(&id, args);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        idstore::invalid_credential_id("".to_string()).code()
    );
}

#[test]
/// Verify we can fetch ID from the recall phrase
fn get_from_recall_phrase() {
    let SetupWithStore {
        module_impl,
        cred_id,
        public_key,
        recall_phrase,
        ..
    } = setup_with_store();
    let result =
        module_impl.get_from_recall_phrase(idstore::GetFromRecallPhraseArgs(recall_phrase));
    assert!(result.is_ok());
    let get_returns = result.unwrap();
    assert_eq!(get_returns.cred_id, cred_id);
    assert_eq!(get_returns.public_key, public_key);
}

#[test]
/// Verify we can't fetch ID from an invalid recall phrase
fn get_from_invalid_recall_phrase() {
    let SetupWithStore { module_impl, .. } = setup_with_store();
    let result = module_impl
        .get_from_recall_phrase(idstore::GetFromRecallPhraseArgs(vec!["Foo".to_string()]));
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        idstore::entry_not_found("".to_string()).code()
    );
}

#[test]
/// Verify we can fetch ID from the public address
fn get_from_address() {
    let SetupWithStore {
        module_impl,
        id,
        cred_id,
        public_key,
        ..
    } = setup_with_store();
    let result = module_impl.get_from_address(idstore::GetFromAddressArgs(id));
    assert!(result.is_ok());
    let get_returns = result.unwrap();
    assert_eq!(get_returns.cred_id, cred_id);
    assert_eq!(get_returns.public_key, public_key);
}

#[test]
/// Verify we can't fetch ID from an invalid address
fn get_from_invalid_address() {
    let SetupWithStore { module_impl, .. } = setup_with_store();
    let result = module_impl.get_from_address(idstore::GetFromAddressArgs(Address::anonymous()));
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().code(),
        idstore::entry_not_found("".to_string()).code()
    );
}
