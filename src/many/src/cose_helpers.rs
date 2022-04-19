use coset::{
    cbor::value::Value,
    iana::{Ec2KeyParameter, EnumI64, OkpKeyParameter},
    Algorithm, CoseKey, KeyOperation, KeyType, Label,
};
use std::collections::{BTreeMap, BTreeSet};

/// Build an EdDSA CoseKey
///
/// # Arguments
///
/// * `x` - Public key
/// * `d` - Private key
pub fn eddsa_cose_key(x: Option<Value>, d: Option<Value>) -> CoseKey {
    // Allocate at least the public key
    let mut params: Vec<(Label, Value)> = Vec::with_capacity(1);
    let mut key_ops: BTreeSet<KeyOperation> = BTreeSet::new();

    if let Some(x) = x {
        params.push((Label::Int(coset::iana::OkpKeyParameter::X as i64), x));
        key_ops.insert(KeyOperation::Assigned(coset::iana::KeyOperation::Verify));
    }

    if let Some(d) = d {
        params.push((Label::Int(coset::iana::OkpKeyParameter::D as i64), d));
        key_ops.insert(KeyOperation::Assigned(coset::iana::KeyOperation::Sign));
    }

    // The CoseKeyBuilder is too limited to be used here
    CoseKey {
        kty: KeyType::Assigned(coset::iana::KeyType::OKP),
        alg: Some(Algorithm::Assigned(coset::iana::Algorithm::EdDSA)),
        key_ops,
        params: [
            vec![(
                Label::Int(coset::iana::OkpKeyParameter::Crv as i64),
                Value::from(coset::iana::EllipticCurve::Ed25519 as u64),
            )],
            params,
        ]
        .concat(),
        ..Default::default()
    }
}

/// Build an ECDSA CoseKey
///
/// # Arguments
///
/// * `(x, y)` - Public key
/// * `d` - Private key
pub fn ecdsa_cose_key((x, y): (Option<Value>, Option<Value>), d: Option<Value>) -> CoseKey {
    // Allocate at least the public key
    let mut params: Vec<(Label, Value)> = Vec::with_capacity(2);
    let mut key_ops: BTreeSet<KeyOperation> = BTreeSet::new();

    if let (Some(x), Some(y)) = (x, y) {
        params.push((
            Label::Int(coset::iana::Ec2KeyParameter::X as i64),
            x.clone(),
        ));
        params.push((
            Label::Int(coset::iana::Ec2KeyParameter::Y as i64),
            y.clone(),
        ));
        key_ops.insert(KeyOperation::Assigned(coset::iana::KeyOperation::Verify));
    }

    if let Some(d) = d {
        params.push((
            Label::Int(coset::iana::Ec2KeyParameter::D as i64),
            d.clone(),
        ));
        key_ops.insert(KeyOperation::Assigned(coset::iana::KeyOperation::Sign));
    }

    // The CoseKeyBuilder is too limited to be used here
    CoseKey {
        kty: KeyType::Assigned(coset::iana::KeyType::EC2),
        alg: Some(Algorithm::Assigned(coset::iana::Algorithm::ES256)),
        key_ops,
        params,
        ..Default::default()
    }
}

// TODO: Change the error type
pub fn public_key(key: &CoseKey) -> Result<CoseKey, String> {
    let params = BTreeMap::from_iter(key.params.clone().into_iter());
    match key.alg {
        Some(Algorithm::Assigned(coset::iana::Algorithm::EdDSA)) => {
            let x = params.get(&Label::Int(OkpKeyParameter::X.to_i64()));
            if x.is_some() {
                Ok(eddsa_cose_key(x.cloned(), None))
            } else {
                Err("Key doesn't have a public key".to_string())
            }
        }
        Some(Algorithm::Assigned(coset::iana::Algorithm::ES256)) => {
            let x = params.get(&Label::Int(Ec2KeyParameter::X.to_i64()));
            let y = params.get(&Label::Int(Ec2KeyParameter::Y.to_i64()));

            if x.is_some() && y.is_some() {
                Ok(ecdsa_cose_key((x.cloned(), y.cloned()), None))
            } else {
                Err("Key doesn't have a public key".to_string())
            }
        }
        _ => Err("Unknown algorithm".to_string()),
    }
}
