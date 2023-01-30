use many_identity::testing::identity;
use many_kvstore::error;
use many_kvstore::module::KvStoreModuleImpl;
use many_modules::kvstore::{GetArgs, KvStoreCommandsModuleBackend, KvStoreModuleBackend, PutArgs};

/// Verify persistent storage can be re-loaded
#[test]
fn load() {
    let path = tempfile::tempdir().unwrap().into_path();
    // Storage needs to become out-of-scope so it can be re-opened
    {
        println!("{}", identity(1));
        let init = r#"{
            // The identity that is used to create new accounts. The server does not need to
            // know the private key for this, just that it's not reused.
            identity: "mahukzwuwgt3porn6q4vq4xu3mwy5gyskhouryzbscq7wb2iow",

            // The initial database ACL
            // This is identity(1)
            acl: {
              "010203": { owner: "maeaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaiye" }
            }
        }"#;
        let state = json5::from_str(init).unwrap();
        let mut module_impl = KvStoreModuleImpl::new(state, path.clone(), false).unwrap();

        // Put some data at some non-used key
        module_impl
            .put(
                &identity(1),
                PutArgs {
                    key: vec![2, 3, 4].into(),
                    value: vec![0, 1, 2, 3].into(),
                    alternative_owner: None,
                },
            )
            .expect("Unable to put new data in DB");
    }

    let mut module_impl = KvStoreModuleImpl::load(path, false).unwrap();

    // Get the data from the previous put
    let v = module_impl
        .get(
            &identity(1),
            GetArgs {
                key: vec![2, 3, 4].into(),
            },
        )
        .unwrap()
        .value
        .unwrap();
    assert_eq!(v, vec![0, 1, 2, 3].into());

    // Put some data in a location protected by the ACL
    // This should fail since the sender is not the key owner
    let p = module_impl.put(
        &identity(2),
        PutArgs {
            key: vec![1, 2, 3].into(),
            value: vec![0].into(),
            alternative_owner: None,
        },
    );
    assert!(p.is_err());
    assert_eq!(p.unwrap_err().code(), error::permission_denied().code());

    // Put some data in a location protected by the ACL
    // This should succeed since the sender is the key owner
    let p = module_impl.put(
        &identity(1),
        PutArgs {
            key: vec![1, 2, 3].into(),
            value: vec![0].into(),
            alternative_owner: None,
        },
    );
    assert!(p.is_ok());

    // Value at the protected location have been updated correctly
    let v = module_impl
        .get(
            &identity(1),
            GetArgs {
                key: vec![1, 2, 3].into(),
            },
        )
        .unwrap()
        .value
        .unwrap();
    assert_eq!(v, vec![0].into());
}
