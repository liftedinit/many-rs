use crate::{Address, Identity};
use coset::cbor::value::Value;
use coset::{AsCborValue, CborSerializable, CoseKey, CoseKeySet, CoseSign1, Label};
use many_error::ManyError;
use sha3::{Digest, Sha3_224};

/// Returns the address of a public COSE key.
///
/// # Safety
/// This methods DOES NOT VERIFY that the cose key is of a public key. There are
/// strict criteria (see spec) for how to define the public key of a COSE Key.
pub unsafe fn address_unchecked(cose_key: &CoseKey) -> Result<Address, ManyError> {
    let pk = Sha3_224::digest(
        cose_key
            .clone()
            .to_vec()
            .map_err(|e| ManyError::unknown(e.to_string()))?,
    );

    Ok(Address::public_key_unchecked(pk.into()))
}

/// Add the keyset to the protected headers of a CoseSign1 envelope, adding to
/// it instead of replacing if it was already present.
pub fn add_keyset_header(
    mut envelope: CoseSign1,
    key: &impl Identity,
) -> Result<CoseSign1, ManyError> {
    let mut cose_key = key
        .public_key()
        .ok_or_else(|| ManyError::unknown("Invalid Public Key"))?;
    cose_key.key_id = key.address().to_vec();

    let headers = &mut envelope.protected.header.rest;

    if let Some(index) = headers
        .iter()
        .position(|(k, _)| k == &Label::Text("keyset".to_string()))
    {
        let keyset = &headers[index].1;
        if let Ok(mut keyset) = CoseKeySet::from_cbor_value(keyset.clone()) {
            keyset.0.push(cose_key);
            headers.get_mut(index).unwrap().1 =
                Value::Bytes(keyset.to_vec().map_err(ManyError::unknown)?);
            return Ok(envelope);
        } else {
            headers.remove(index);
        }
    }

    let mut keyset = CoseKeySet::default();
    keyset.0.push(cose_key);

    envelope.protected.header.rest.push((
        Label::Text("keyset".to_string()),
        Value::Bytes(keyset.to_vec().map_err(ManyError::unknown)?),
    ));

    Ok(envelope)
}

/// Extract the keyset parameter from the envelope.
pub fn keyset_from_cose_sign1(envelope: &CoseSign1) -> Option<CoseKeySet> {
    let keyset = &envelope
        .protected
        .header
        .rest
        .iter()
        .find(|(k, _)| k == &coset::Label::Text("keyset".to_string()))?
        .1;

    let bytes = keyset.as_bytes()?;
    CoseKeySet::from_slice(bytes).ok()
}
