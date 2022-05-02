// TODO: Implement a SubresourceId type. Best way here is to use num-* crates.
use crate::cose_helpers::public_key;
use crate::message::ManyError;
use coset::{CborSerializable, CoseKey};
use minicbor::data::Type;
use minicbor::encode::Write;
use minicbor::{Decode, Decoder, Encode, Encoder};
use serde::Deserialize;
use sha3::digest::generic_array::typenum::Unsigned;
use sha3::{Digest, Sha3_224};
use std::convert::TryFrom;
use std::fmt::{Debug, Formatter};
use std::str::FromStr;

pub mod cose;
pub use cose::CoseKeyIdentity;

/// Subresource IDs are 31 bit integers.
pub const MAX_SUBRESOURCE_ID: u32 = 0x7FFF_FFFF;

const MAX_IDENTITY_BYTE_LEN: usize = 32;
const SHA_OUTPUT_SIZE: usize = <Sha3_224 as Digest>::OutputSize::USIZE;
pub type PublicKeyHash = [u8; SHA_OUTPUT_SIZE];

/// An identity in the TBD-Verse. This could be a server, network, user, DAO, automated
/// process, etc.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[must_use]
pub struct Identity(InnerIdentity);

impl Identity {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ManyError> {
        InnerIdentity::try_from(bytes).map(Self)
    }

    pub const fn anonymous() -> Self {
        Self(InnerIdentity::anonymous())
    }

    pub fn public_key(key: &CoseKey) -> Self {
        let pk = Sha3_224::digest(&public_key(key).unwrap().to_vec().unwrap());
        Self(InnerIdentity::public_key(pk.into()))
    }

    pub fn subresource(key: &CoseKey, subid: u32) -> Result<Self, ManyError> {
        if subid > MAX_SUBRESOURCE_ID {
            Err(ManyError::invalid_identity_subid())
        } else {
            let pk = Sha3_224::digest(&public_key(key).unwrap().to_vec().unwrap());
            Ok(Self(InnerIdentity::subresource_unchecked(pk.into(), subid)))
        }
    }

    pub const fn is_anonymous(&self) -> bool {
        self.0.is_anonymous()
    }
    pub const fn is_public_key(&self) -> bool {
        self.0.is_public_key()
    }
    pub const fn is_subresource(&self) -> bool {
        self.0.is_subresource()
    }

    pub const fn subresource_id(&self) -> Option<u32> {
        self.0.subresource_id()
    }

    pub fn with_subresource_id(&self, subid: u32) -> Result<Self, ManyError> {
        if subid > MAX_SUBRESOURCE_ID {
            Err(ManyError::invalid_identity_subid())
        } else {
            Ok(self.with_subresource_id_unchecked(subid))
        }
    }

    pub const fn with_subresource_id_unchecked(&self, subid: u32) -> Self {
        if let Some(h) = self.0.hash() {
            Self(InnerIdentity::subresource_unchecked(h, subid))
        } else {
            Self::anonymous()
        }
    }

    pub const fn can_sign(&self) -> bool {
        self.is_public_key() || self.is_subresource()
    }

    pub const fn can_be_source(&self) -> bool {
        self.is_anonymous() || self.is_public_key() || self.is_subresource()
    }

    pub const fn can_be_dest(&self) -> bool {
        self.is_public_key() || self.is_subresource()
    }

    pub fn to_vec(self) -> Vec<u8> {
        self.0.to_vec()
    }

    pub fn to_byte_array(self) -> [u8; MAX_IDENTITY_BYTE_LEN] {
        self.0.to_byte_array()
    }

    pub fn matches_key(&self, key: Option<&CoseKey>) -> bool {
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

    /// Check that another identity matches this one, ignoring any subresouce IDs.
    pub fn matches(&self, other: &Identity) -> bool {
        if self.is_anonymous() {
            other.is_anonymous()
        } else {
            // Extract public key hash of both.
            self.0.hash() == other.0.hash()
        }
    }
}

#[cfg(feature = "raw")]
impl Identity {
    /// Create an identity from the raw value of a public key hash, without checking
    /// its validity.
    #[inline(always)]
    pub fn public_key_raw(hash: PublicKeyHash) -> Self {
        Self(InnerIdentity::public_key(hash))
    }

    /// Create an identity from the raw value of a public key hash and a subresource
    /// id. The hash isn't validated, but the subid is.
    #[inline(always)]
    pub fn subresource_raw(hash: PublicKeyHash, subid: u32) -> Self {
        Self(InnerIdentity::subresource_unchecked(hash, subid))
    }
}

impl PartialEq<&str> for Identity {
    #[allow(clippy::cmp_owned)]
    fn eq(&self, other: &&str) -> bool {
        self.to_string() == *other
    }
}

impl Debug for Identity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Identity")
            .field(&if self.is_anonymous() {
                "anonymous".to_string()
            } else if self.is_public_key() {
                "public-key".to_string()
            } else if self.is_subresource() {
                format!("subresource({})", self.subresource_id().unwrap_or_default())
            } else {
                "??".to_string()
            })
            .field(&self.to_string())
            .finish()
    }
}

impl Default for Identity {
    fn default() -> Self {
        Identity::anonymous()
    }
}

impl std::fmt::Display for Identity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl Encode for Identity {
    fn encode<W: Write>(
        &self,
        e: &mut Encoder<W>,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.tag(minicbor::data::Tag::Unassigned(10000))?
            .bytes(&self.to_vec())?;
        Ok(())
    }
}

impl<'b> Decode<'b> for Identity {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let mut is_tagged = false;
        // Check all the tags.
        while d.datatype()? == Type::Tag {
            if d.tag()? == minicbor::data::Tag::Unassigned(10000) {
                is_tagged = true;
            }
        }

        match d.datatype()? {
            Type::String => Self::from_str(d.str()?),
            _ => {
                if !is_tagged {
                    return Err(minicbor::decode::Error::Message(
                        "identities need to be tagged",
                    ));
                }

                Self::try_from(d.bytes()?)
            }
        }
        .map_err(|_e| minicbor::decode::Error::Message("Could not decode identity from bytes"))
    }
}

impl<'de> Deserialize<'de> for Identity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Identity;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("identity string or bytes")
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Identity::from_str(v).map_err(E::custom)
            }

            fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Identity::from_bytes(v).map_err(E::custom)
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(Visitor)
        } else {
            deserializer.deserialize_byte_buf(Visitor)
        }
    }
}

impl TryFrom<&[u8]> for Identity {
    type Error = ManyError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl TryFrom<String> for Identity {
    type Error = ManyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        InnerIdentity::try_from(value).map(Self)
    }
}

impl FromStr for Identity {
    type Err = ManyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        InnerIdentity::from_str(s).map(Self)
    }
}

impl AsRef<[u8; MAX_IDENTITY_BYTE_LEN]> for Identity {
    fn as_ref(&self) -> &[u8; MAX_IDENTITY_BYTE_LEN] {
        let result: &[u8; MAX_IDENTITY_BYTE_LEN] = unsafe { std::mem::transmute(self) };
        result
    }
}

#[derive(Copy, Clone, Eq, Debug, Ord, PartialOrd)]
#[non_exhaustive]
#[must_use]
struct InnerIdentity {
    bytes: [u8; MAX_IDENTITY_BYTE_LEN],
}

// Identity needs to be bound to 32 bytes maximum.
static_assertions::assert_eq_size!([u8; MAX_IDENTITY_BYTE_LEN], InnerIdentity);
static_assertions::const_assert_eq!(InnerIdentity::anonymous().to_byte_array()[0], 0);

impl PartialEq for InnerIdentity {
    fn eq(&self, other: &Self) -> bool {
        match (&self.bytes[0], &other.bytes[0]) {
            // Anonymous
            (0, 0) => true,

            // Public Key
            (1, 1) => self.bytes[1..=SHA_OUTPUT_SIZE] == other.bytes[1..=SHA_OUTPUT_SIZE],

            // Subresource
            (x @ 0x80..=0xFF, y @ 0x80..=0xFF) if x == y => self.bytes[1..] == other.bytes[1..],

            // Anything else if by default inequal.
            (_, _) => false,
        }
    }
}

impl Default for InnerIdentity {
    fn default() -> Self {
        InnerIdentity::anonymous()
    }
}

impl InnerIdentity {
    pub const fn anonymous() -> Self {
        Self {
            bytes: [0; MAX_IDENTITY_BYTE_LEN],
        }
    }

    pub const fn public_key(hash: [u8; SHA_OUTPUT_SIZE]) -> Self {
        let mut bytes = [0; MAX_IDENTITY_BYTE_LEN];
        bytes[0] = 1;
        let mut len = SHA_OUTPUT_SIZE;
        while len > 0 {
            len -= 1;
            bytes[1 + len] = hash[len];
        }
        Self { bytes }
    }

    pub(crate) const fn subresource_unchecked(hash: [u8; SHA_OUTPUT_SIZE], id: u32) -> Self {
        // Get a public key and add the resource id.
        let mut bytes = Self::public_key(hash).bytes;
        bytes[0] = 0x80 + ((id & 0x7F00_0000) >> 24) as u8;
        bytes[(SHA_OUTPUT_SIZE + 1)] = ((id & 0x00FF_0000) >> 16) as u8;
        bytes[(SHA_OUTPUT_SIZE + 2)] = ((id & 0x0000_FF00) >> 8) as u8;
        bytes[(SHA_OUTPUT_SIZE + 3)] = (id & 0x0000_00FF) as u8;
        Self { bytes }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ManyError> {
        let bytes = bytes;
        if bytes.is_empty() {
            return Err(ManyError::invalid_identity());
        }

        match bytes[0] {
            0 => {
                if bytes.len() > 1 {
                    Err(ManyError::invalid_identity())
                } else {
                    Ok(Self::anonymous())
                }
            }
            1 => {
                if bytes.len() != 29 {
                    Err(ManyError::invalid_identity())
                } else {
                    let mut slice = [0; 28];
                    slice.copy_from_slice(&bytes[1..29]);
                    Ok(Self::public_key(slice))
                }
            }
            hi @ 0x80..=0xff => {
                if bytes.len() != 32 {
                    Err(ManyError::invalid_identity())
                } else {
                    let mut hash = [0; 28];
                    let mut subid = [0; 4];
                    hash.copy_from_slice(&bytes[1..29]);
                    subid[0] = hi;
                    subid[1..].copy_from_slice(&bytes[29..32]);
                    Ok(Self::subresource_unchecked(hash, u32::from_be_bytes(subid)))
                }
            }
            x => Err(ManyError::invalid_identity_kind(x.to_string())),
        }
    }

    pub fn from_str(value: &str) -> Result<Self, ManyError> {
        if !value.starts_with('m') {
            return Err(ManyError::invalid_identity_prefix(value[0..0].to_string()));
        }

        if &value[1..] == "aa" || &value[1..] == "aaaa" {
            Ok(Self::anonymous())
        } else {
            let data = &value[..value.len() - 2][1..];
            let data = base32::decode(base32::Alphabet::RFC4648 { padding: false }, data).unwrap();
            let result = Self::try_from(data.as_slice())?;

            if result.to_string() != value {
                Err(ManyError::invalid_identity())
            } else {
                Ok(result)
            }
        }
    }

    pub const fn to_byte_array(self) -> [u8; MAX_IDENTITY_BYTE_LEN] {
        self.bytes
    }

    #[rustfmt::skip]
    pub fn to_vec(self) -> Vec<u8> {
        // This makes sure we actually have a Vec<u8> that's smaller than 32 bytes if
        // it can be.
        match self.bytes[0] {
            0 => vec![0],
            1 => {
                let pk = &self.bytes[1..=SHA_OUTPUT_SIZE];
                vec![
                    1,
                    pk[ 0], pk[ 1], pk[ 2], pk[ 3], pk[ 4], pk[ 5], pk[ 6], pk[ 7],
                    pk[ 8], pk[ 9], pk[10], pk[11], pk[12], pk[13], pk[14], pk[15],
                    pk[16], pk[17], pk[18], pk[19], pk[20], pk[21], pk[22], pk[23],
                    pk[24], pk[25], pk[26], pk[27],
                ]
            }
            0x80..=0xFF => {
                self.bytes.to_vec()
            }
            _ => unreachable!(),
        }
    }

    pub const fn is_anonymous(&self) -> bool {
        self.bytes[0] == 0
    }
    pub const fn is_public_key(&self) -> bool {
        self.bytes[0] == 1
    }
    pub const fn is_subresource(&self) -> bool {
        matches!(self.bytes[0], 0x80..=0xFF)
    }

    pub const fn subresource_id(&self) -> Option<u32> {
        match self.bytes[0] {
            x @ 0x80..=0xFF => {
                let high = ((x & 0x7F) as u32) << 24;
                let mut low = (self.bytes[SHA_OUTPUT_SIZE + 1] as u32) << 16;
                low += (self.bytes[SHA_OUTPUT_SIZE + 2] as u32) << 8;
                low += self.bytes[SHA_OUTPUT_SIZE + 3] as u32;
                Some(high + low)
            }
            _ => None,
        }
    }

    pub const fn hash(&self) -> Option<[u8; SHA_OUTPUT_SIZE]> {
        match self.bytes[0] {
            1 | 0x80..=0xFF => {
                let mut hash = [0; SHA_OUTPUT_SIZE];
                let mut len = SHA_OUTPUT_SIZE;
                while len > 0 {
                    len -= 1;
                    hash[len] = self.bytes[1 + len];
                }
                Some(hash)
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for InnerIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let data = self.to_vec();
        let mut crc = crc_any::CRCu16::crc16();
        crc.digest(&data);

        let crc = crc.get_crc().to_be_bytes();
        write!(
            f,
            "m{}{}",
            base32::encode(base32::Alphabet::RFC4648 { padding: false }, &data)
                .to_ascii_lowercase(),
            base32::encode(base32::Alphabet::RFC4648 { padding: false }, &crc)
                .get(0..2)
                .unwrap()
                .to_ascii_lowercase(),
        )
    }
}

impl TryFrom<String> for InnerIdentity {
    type Error = ManyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        InnerIdentity::from_str(value.as_str())
    }
}

impl TryFrom<&[u8]> for InnerIdentity {
    type Error = ManyError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

#[cfg(feature = "serde")]
mod serde {
    use crate::identity::{Identity, InnerIdentity};
    use serde::Deserialize;
    use std::fmt::Formatter;

    impl serde::ser::Serialize for Identity {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::ser::Serializer,
        {
            if serializer.is_human_readable() {
                serializer.serialize_str(&self.0.to_string())
            } else {
                serializer.serialize_bytes(&self.0.to_vec())
            }
        }
    }

    impl<'de> serde::ser::Deserialize<'de> for Identity {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::ser::Deserializer<'de>,
        {
            let inner = InnerIdentity::deserialize(deserializer)?;
            Ok(Self(inner))
        }
    }

    struct HumanReadableInnerIdentityVisitor;

    impl serde::de::Visitor<'_> for HumanReadableInnerIdentityVisitor {
        type Value = InnerIdentity;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("a textual MANY identity")
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            InnerIdentity::from_str(v.as_str()).map_err(E::custom)
        }
    }

    struct InnerIdentityVisitor;

    impl serde::de::Visitor<'_> for InnerIdentityVisitor {
        type Value = InnerIdentity;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("a byte buffer")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            InnerIdentity::from_bytes(v).map_err(E::custom)
        }
    }

    impl<'de> serde::de::Deserialize<'de> for InnerIdentity {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            if deserializer.is_human_readable() {
                deserializer.deserialize_string(HumanReadableInnerIdentityVisitor)
            } else {
                deserializer.deserialize_bytes(InnerIdentityVisitor)
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::types::identity::cose::tests::{ecdsa_256_identity, eddsa_identity};
    use crate::Identity;
    use std::str::FromStr;

    pub fn identity(seed: u32) -> Identity {
        #[rustfmt::skip]
        let bytes = [
            1u8,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            (seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8, (seed & 0xFF) as u8
        ];
        Identity::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn can_read_anonymous() {
        let a = Identity::anonymous();
        let a_str = a.to_string();
        let a2 = Identity::from_str(&a_str).unwrap();

        assert_eq!(a, a2);
    }

    #[test]
    fn byte_array_conversion() {
        let a = Identity::anonymous();
        let b = identity(1);
        let c = identity(2);

        assert_ne!(a.to_string(), b.to_string());
        assert_ne!(b.to_string(), c.to_string());
        assert_ne!(a.to_vec(), b.to_vec());
        assert_ne!(b.to_vec(), c.to_vec());

        assert_eq!(Identity::from_str(&a.to_string()), Ok(a));
        assert_eq!(Identity::from_str(&b.to_string()), Ok(b));
        assert_eq!(Identity::from_str(&c.to_string()), Ok(c));
    }

    #[test]
    fn textual_format_1() {
        let a = Identity::from_str("mahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqys").unwrap();
        let b = Identity::from_bytes(
            &hex::decode("01c8aead03f915f128f0fa7ff696c656eaa93db87bd9aa73df693acb22").unwrap(),
        )
        .unwrap();

        assert_eq!(a, b);
    }

    #[test]
    fn textual_format_2() {
        let a =
            Identity::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap();
        let b = Identity::from_bytes(
            &hex::decode("804a101d521d810211a0c6346ba89bd1cc1f821c03b969ff9d5c8b2f59000001")
                .unwrap(),
        )
        .unwrap();

        assert_eq!(a, b);
    }

    #[test]
    fn subresource_1() {
        let a = Identity::from_str("mahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqys")
            .unwrap()
            .with_subresource_id(1)
            .unwrap();
        let b = Identity::from_bytes(
            &hex::decode("80c8aead03f915f128f0fa7ff696c656eaa93db87bd9aa73df693acb22000001")
                .unwrap(),
        )
        .unwrap();
        let c = Identity::from_bytes(
            &hex::decode("80c8aead03f915f128f0fa7ff696c656eaa93db87bd9aa73df693acb22000002")
                .unwrap(),
        )
        .unwrap();

        assert_eq!(a, b);
        assert_eq!(b.with_subresource_id(2).unwrap(), c);
    }

    proptest::proptest! {
        #[test]
        fn subresource_id_fuzzy(subid: u32) {
            let a = Identity::from_str("mahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqys")
                .unwrap()
                .with_subresource_id(subid);

            if let Ok(id) = a {
                let b = Identity::from_str(&id.to_string());
                assert_eq!(a, b);
            } else {
                assert_eq!(subid.leading_zeros(), 0);
            }
        }
    }

    #[test]
    fn from_pem_eddsa() {
        let id = eddsa_identity();
        assert_eq!(
            id.identity,
            "maffbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wijp"
        );
    }

    #[test]
    fn from_pem_ecdsa() {
        let id = ecdsa_256_identity();
        assert_eq!(
            id.identity,
            "magcncsncbfmfdvezjmfick47pwgefjnm6zcaghu7ffe3o3qtf"
        );
    }

    #[test]
    fn matches_key() {
        let id = eddsa_identity();
        assert!(id.identity.matches_key(id.key.as_ref()));
    }
}
