use many_identity::{AcceptAllVerifier, Address, AnonymousIdentity, Identity};
use many_identity_dsa::ed25519::generate_random_ed25519_identity;
use many_modules::delegation::attributes::DelegationAttribute;
use many_modules::delegation::{DelegationModule, DelegationModuleBackend, WhoAmIReturn};
use many_protocol::{
    decode_response_from_cose_sign1, encode_cose_sign1_from_request, BaseIdentityResolver,
    RequestMessage, RequestMessageBuilder,
};
use many_server::transport::LowLevelManyRequestHandler;
use many_types::delegation::Certificate;
use many_types::Timestamp;
use std::sync::{Arc, Mutex};

#[test]
fn simple() {
    let server = many_server::ManyServer::test(AnonymousIdentity);
    {
        struct DelegationImpl;
        impl DelegationModuleBackend for DelegationImpl {}

        let mut server_locked = server.lock().unwrap();
        let module = DelegationModule::new(Arc::new(Mutex::new(DelegationImpl)));
        server_locked.add_module(module);
    }

    let id1 = generate_random_ed25519_identity();
    let id2 = generate_random_ed25519_identity();
    let delegation = Certificate::new(id1.address(), id2.address(), Timestamp::now() + 100)
        .sign(&id1)
        .unwrap();
    let request: RequestMessage = RequestMessageBuilder::default()
        .from(id1.address())
        .to(Address::anonymous())
        .method("delegation.whoAmI".to_string())
        .build()
        .unwrap()
        .with_attribute(
            DelegationAttribute::new(vec![delegation])
                .try_into()
                .unwrap(),
        );

    let envelope = encode_cose_sign1_from_request(request, &id2).unwrap();
    let response_e = smol::block_on(server.execute(envelope)).unwrap();
    let response = decode_response_from_cose_sign1(
        &response_e,
        None,
        &AcceptAllVerifier,
        &BaseIdentityResolver,
    )
    .unwrap();

    let result = response.decode::<WhoAmIReturn>().unwrap();
    assert_eq!(result.address, id1.address());
}

#[test]
fn chained() {
    let server = many_server::ManyServer::test(AnonymousIdentity);
    {
        struct DelegationImpl;
        impl DelegationModuleBackend for DelegationImpl {}

        let mut server_locked = server.lock().unwrap();
        let module = DelegationModule::new(Arc::new(Mutex::new(DelegationImpl)));
        server_locked.add_module(module);
    }

    let id1 = generate_random_ed25519_identity();
    let id2 = generate_random_ed25519_identity();
    let id3 = generate_random_ed25519_identity();
    let delegation1_2 = Certificate::new(id1.address(), id2.address(), Timestamp::now() + 100)
        .sign(&id1)
        .unwrap();
    let delegation2_3 = Certificate::new(id2.address(), id3.address(), Timestamp::now() + 100)
        .sign(&id2)
        .unwrap();

    let request: RequestMessage = RequestMessageBuilder::default()
        .from(id1.address())
        .to(Address::anonymous())
        .method("delegation.whoAmI".to_string())
        .build()
        .unwrap()
        .with_attribute(
            DelegationAttribute::new(vec![delegation2_3, delegation1_2])
                .try_into()
                .unwrap(),
        );

    let envelope = encode_cose_sign1_from_request(request, &id3).unwrap();
    let response_e = smol::block_on(server.execute(envelope)).unwrap();
    let response = decode_response_from_cose_sign1(
        &response_e,
        None,
        &AcceptAllVerifier,
        &BaseIdentityResolver,
    )
    .unwrap();

    let result = response.decode::<WhoAmIReturn>().unwrap();
    assert_eq!(result.address, id1.address());
}

#[test]
fn is_final() {
    let server = many_server::ManyServer::test(AnonymousIdentity);
    {
        struct DelegationImpl;
        impl DelegationModuleBackend for DelegationImpl {}

        let mut server_locked = server.lock().unwrap();
        let module = DelegationModule::new(Arc::new(Mutex::new(DelegationImpl)));
        server_locked.add_module(module);
    }

    let id1 = generate_random_ed25519_identity();
    let id2 = generate_random_ed25519_identity();
    let id3 = generate_random_ed25519_identity();
    let delegation1_2 = Certificate::new(id1.address(), id2.address(), Timestamp::now() + 100)
        .with_final(true)
        .sign(&id1)
        .unwrap();
    let delegation2_3 = Certificate::new(id2.address(), id3.address(), Timestamp::now() + 100)
        .sign(&id2)
        .unwrap();

    let request: RequestMessage = RequestMessageBuilder::default()
        .from(id1.address())
        .to(Address::anonymous())
        .method("delegation.whoAmI".to_string())
        .build()
        .unwrap()
        .with_attribute(
            DelegationAttribute::new(vec![delegation2_3, delegation1_2])
                .try_into()
                .unwrap(),
        );

    let envelope = encode_cose_sign1_from_request(request, &id3).unwrap();
    let response_e = smol::block_on(server.execute(envelope)).unwrap();
    let response = decode_response_from_cose_sign1(
        &response_e,
        None,
        &AcceptAllVerifier,
        &BaseIdentityResolver,
    )
    .unwrap();

    let result = response.decode::<WhoAmIReturn>().unwrap();
    assert_eq!(result.address, id1.address());
}

#[test]
fn invalid_is_final() {
    let server = many_server::ManyServer::test(AnonymousIdentity);
    {
        struct DelegationImpl;
        impl DelegationModuleBackend for DelegationImpl {}

        let mut server_locked = server.lock().unwrap();
        let module = DelegationModule::new(Arc::new(Mutex::new(DelegationImpl)));
        server_locked.add_module(module);
    }

    let id1 = generate_random_ed25519_identity();
    let id2 = generate_random_ed25519_identity();
    let id3 = generate_random_ed25519_identity();
    let delegation1_2 = Certificate::new(id1.address(), id2.address(), Timestamp::now() + 100)
        .sign(&id1)
        .unwrap();
    let delegation2_3 = Certificate::new(id2.address(), id3.address(), Timestamp::now() + 100)
        .with_final(true)
        .sign(&id2)
        .unwrap();

    let request: RequestMessage = RequestMessageBuilder::default()
        .from(id1.address())
        .to(Address::anonymous())
        .method("delegation.whoAmI".to_string())
        .build()
        .unwrap()
        .with_attribute(
            DelegationAttribute::new(vec![delegation2_3, delegation1_2])
                .try_into()
                .unwrap(),
        );

    let envelope = encode_cose_sign1_from_request(request, &id3).unwrap();
    let response_e = smol::block_on(server.execute(envelope)).unwrap();
    let response = decode_response_from_cose_sign1(
        &response_e,
        None,
        &AcceptAllVerifier,
        &BaseIdentityResolver,
    )
    .unwrap();

    assert!(response.data.is_err());
}
