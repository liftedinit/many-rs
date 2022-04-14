use coset::{
    cbor::value::Value,
    iana::{Ec2KeyParameter, EnumI64, OkpKeyParameter},
    Algorithm, CoseKey, KeyOperation, KeyType, Label,
};
use std::collections::{BTreeMap, BTreeSet};

// TODO: Change the error type
pub fn public_key(key: &CoseKey) -> Result<CoseKey, String> {
    // The minicose version of this code discards any error and simply return None
    let params = BTreeMap::from_iter(key.params.clone().into_iter());
    match key.alg {
        Some(Algorithm::Assigned(coset::iana::Algorithm::EdDSA)) => {
            let x = params.get(&Label::Int(OkpKeyParameter::X.to_i64()));
            if let Some(x) = x {
                // The CoseKeyBuilder is too limited to be used here
                let cose_key = CoseKey {
                    kty: KeyType::Assigned(coset::iana::KeyType::OKP),
                    alg: Some(Algorithm::Assigned(coset::iana::Algorithm::EdDSA)),
                    key_ops: BTreeSet::from([KeyOperation::Assigned(
                        coset::iana::KeyOperation::Verify,
                    )]),
                    params: vec![
                        (
                            Label::Int(coset::iana::Ec2KeyParameter::Crv as i64),
                            Value::from(coset::iana::EllipticCurve::Ed25519 as u64),
                        ),
                        (
                            Label::Int(coset::iana::OkpKeyParameter::X as i64),
                            x.clone(),
                        ),
                    ],
                    ..Default::default()
                };
                Ok(cose_key)
            } else {
                Err("Key doesn't have a public key".to_string())
            }
        }
        Some(Algorithm::Assigned(coset::iana::Algorithm::ES256)) => {
            let x = params.get(&Label::Int(Ec2KeyParameter::X.to_i64()));
            let y = params.get(&Label::Int(Ec2KeyParameter::Y.to_i64()));

            if let (Some(x), Some(y)) = (x, y) {
                // The CoseKeyBuilder is too limited to be used here
                let cose_key = CoseKey {
                    kty: KeyType::Assigned(coset::iana::KeyType::EC2),
                    alg: Some(Algorithm::Assigned(coset::iana::Algorithm::ES256)),
                    key_ops: BTreeSet::from([KeyOperation::Assigned(
                        coset::iana::KeyOperation::Verify,
                    )]),
                    params: vec![
                        (
                            Label::Int(coset::iana::Ec2KeyParameter::X as i64),
                            x.clone(),
                        ),
                        (
                            Label::Int(coset::iana::Ec2KeyParameter::Y as i64),
                            y.clone(),
                        ),
                    ],
                    ..Default::default()
                };
                Ok(cose_key)
            } else {
                Err("Key doesn't have a public key".to_string())
            }
        }
        _ => Err("Unknown algorithm".to_string()), // Unsupported Algorithm
    }
}
