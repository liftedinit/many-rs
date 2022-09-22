use crate::Timestamp;
use coset::{CoseSign1, CoseSign1Builder};
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};
use minicbor::{Decode, Encode};

/// A delegation certificate.
#[derive(Debug, Encode, Decode, Ord, PartialOrd, Eq, PartialEq)]
#[cbor(map)]
pub struct Certificate {
    /// The address of the delegated identity (`Alice` in the example).
    #[n(0)]
    pub from: Address,

    /// The address of the identity that can use the above identity (`Bob` in the example).
    #[n(1)]
    pub to: Address,

    /// An expiration timestamp. If the system time is past this timestamp, the certificate is
    /// invalid and the server MUST return an error without opening the envelope further.
    #[n(2)]
    pub expiration: Timestamp,
    // TODO: uncomment this when PR #201 is in.
    // #[n(3)]
    // memo: Option<Memo>,
}

impl Certificate {
    pub fn new(from: Address, to: Address, expiration: Timestamp) -> Self {
        Self {
            from,
            to,
            expiration,
        }
    }

    pub fn sign(&self, id: &impl Identity) -> Result<CoseSign1, ManyError> {
        if !self.from.matches(&id.address()) {
            return Err(ManyError::unknown("From does not match identity."));
        }

        // Create the CoseSign1, then sign it.
        let cose_sign_1 = CoseSign1Builder::new()
            .payload(minicbor::to_vec(self).map_err(ManyError::deserialization_error)?)
            .build();

        id.sign_1(cose_sign_1)
    }

    pub fn decode_and_verify(
        envelope: &CoseSign1,
        verifier: &impl Verifier,
        now: Timestamp,
    ) -> Result<Self, ManyError> {
        let from = verifier.verify_1(envelope)?;
        let payload = envelope
            .payload
            .as_ref()
            .ok_or_else(|| ManyError::unknown("Empty envelope."))?;
        let certificate: Self =
            minicbor::decode(payload).map_err(ManyError::deserialization_error)?;

        if !certificate.from.matches(&from) {
            return Err(ManyError::unknown("From does not match identity."));
        }
        if certificate.expiration <= now {
            return Err(ManyError::unknown("Delegation certificate expired."));
        }

        Ok(certificate)
    }
}

#[cfg(test)]
mod tests {
    use super::Certificate;
    use crate::Timestamp;
    use coset::CoseSign1;
    use many_identity::{AnonymousIdentity, Identity};
    use many_identity_dsa::ed25519::generate_random_ed25519_identity;
    use many_identity_dsa::CoseKeyVerifier;

    #[test]
    fn valid() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let certificate = Certificate::new(id1.address(), id2.address(), now + 1000);

        let envelope = certificate.sign(&id1).unwrap();
        let result = Certificate::decode_and_verify(&envelope, &CoseKeyVerifier, Timestamp::now());

        assert_eq!(result, Ok(certificate));
    }

    #[test]
    fn invalid_expiration() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let certificate = Certificate::new(id1.address(), id2.address(), now);

        let envelope = certificate.sign(&id1).unwrap();
        let result = Certificate::decode_and_verify(&envelope, &CoseKeyVerifier, Timestamp::now());

        assert!(result.is_err());
    }

    #[test]
    fn invalid_from_sign() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let certificate = Certificate::new(AnonymousIdentity.address(), id2.address(), now);
        assert!(certificate.sign(&id1).is_err());
    }

    #[test]
    fn invalid_from() {
        let id1 = generate_random_ed25519_identity();
        let id2 = generate_random_ed25519_identity();

        let now = Timestamp::now();
        let certificate = Certificate::new(AnonymousIdentity.address(), id2.address(), now);

        // Create envelope using the wrong signing identity.
        let mut cose_sign_1 = CoseSign1::default();
        cose_sign_1.payload = Some(minicbor::to_vec(certificate).unwrap());
        let envelope = id1.sign_1(cose_sign_1).unwrap();

        let result = Certificate::decode_and_verify(&envelope, &CoseKeyVerifier, Timestamp::now());
        assert!(result.is_err());
    }
}
