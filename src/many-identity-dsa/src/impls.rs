use coset::cbor::value::{Integer, Value};
use coset::iana::{EnumI64, OkpKeyParameter};
use coset::{CoseKey, Label};
use many_error::ManyError;

#[cfg(feature = "ed25519")]
pub mod ed25519;

#[cfg(feature = "ecdsa")]
pub mod ecdsa;

/// Assert a COSE key as valid.
fn check_key(
    cose_key: &CoseKey,
    sign: bool,
    verify: bool,
    key_type: coset::iana::KeyType,
    algo: coset::iana::Algorithm,
    crv: Option<(&str, coset::iana::EllipticCurve)>,
) -> Result<(), ManyError> {
    if sign
        && !cose_key.key_ops.contains(&coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Sign,
        ))
    {
        return Err(ManyError::unknown("Key cannot sign"));
    }
    if verify
        && !cose_key.key_ops.contains(&coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Verify,
        ))
    {
        return Err(ManyError::unknown("Key cannot verify"));
    }

    if cose_key.kty != coset::KeyType::Assigned(key_type) {
        return Err(ManyError::unknown(format!(
            "Wrong key type: {:?}",
            cose_key.kty
        )));
    }
    if cose_key.alg != Some(coset::Algorithm::Assigned(algo)) {
        return Err(ManyError::unknown(format!(
            "Wrong key algorihm: {:?}",
            cose_key.alg
        )));
    }

    if let Some((crv_name, crv)) = crv {
        if cose_key
            .params
            .iter()
            .find(|(k, _v)| k == &Label::Int(OkpKeyParameter::Crv.to_i64()))
            .map(|(_k, v)| v)
            .and_then(Value::as_integer)
            .ok_or_else(|| ManyError::unknown("Crv parameter not found."))?
            != Integer::from(crv.to_i64())
        {
            return Err(ManyError::unknown(format!(
                "Curve unsupported. Expected {crv_name}"
            )));
        }
    }

    Ok(())
}
