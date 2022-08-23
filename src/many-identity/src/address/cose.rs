use crate::address::PublicKeyHash;
use crate::cose_helpers::public_key;
use coset::CborSerializable;
use sha3::{Digest, Sha3_224};

impl crate::Address {
    pub fn matches_key(&self, key: Option<&coset::CoseKey>) -> bool {
        if self.is_anonymous() {
            key.is_none()
        } else if self.is_public_key() || self.is_subresource() {
            if let Some(cose_key) = key {
                let key_hash: PublicKeyHash =
                    Sha3_224::digest(&public_key(cose_key).unwrap().to_vec().unwrap()).into();

                self.0
                    .hash()
                    .unwrap() // TODO: CAN THIS FAIL?
                    .iter()
                    .zip(key_hash.iter())
                    .all(|(a, b)| a == b)
            } else {
                false
            }
        } else {
            false
        }
    }
}
