use many_error::ManyError;
use sha3::digest::generic_array::typenum::Unsigned;
use sha3::digest::OutputSizeUser;
use sha3::Sha3_224;
use std::convert::TryFrom;
use std::fmt::{Debug, Formatter};
use std::str::FromStr;

#[cfg(feature = "minicbor")]
mod minicbor;

#[cfg(feature = "serde")]
mod serde;

/// Subresource IDs are 31 bit integers.
pub const MAX_SUBRESOURCE_ID: u32 = 0x7FFF_FFFF;

const MAX_IDENTITY_BYTE_LEN: usize = 32;
const SHA_OUTPUT_SIZE: usize = <Sha3_224 as OutputSizeUser>::OutputSize::USIZE;
pub type PublicKeyHash = [u8; SHA_OUTPUT_SIZE];

/// A subresource ID. Addresses with this must be of type 0x80-0xFF.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[must_use]
pub struct SubresourceId(pub(crate) u32);

impl SubresourceId {
    /// The discriminant for an address with this subresource ID.
    /// This will be in the range of 0x80..=0xFF.
    pub const fn discriminant(&self) -> u8 {
        0x80 + ((self.0 & 0x7F00_0000) >> 24) as u8
    }
}

impl TryInto<SubresourceId> for i16 {
    type Error = ManyError;

    fn try_into(self) -> Result<SubresourceId, Self::Error> {
        (self as i32).try_into()
    }
}

impl TryInto<SubresourceId> for i32 {
    type Error = ManyError;

    fn try_into(self) -> Result<SubresourceId, Self::Error> {
        if self < 0 {
            Err(ManyError::invalid_identity_subid())
        } else {
            Ok(SubresourceId(self as u32))
        }
    }
}

impl TryInto<SubresourceId> for u16 {
    type Error = ManyError;

    fn try_into(self) -> Result<SubresourceId, Self::Error> {
        Ok(SubresourceId(self as u32))
    }
}

impl TryInto<SubresourceId> for u32 {
    type Error = ManyError;

    fn try_into(self) -> Result<SubresourceId, Self::Error> {
        if self > MAX_SUBRESOURCE_ID {
            Err(ManyError::invalid_identity_subid())
        } else {
            Ok(SubresourceId(self))
        }
    }
}

impl TryInto<SubresourceId> for u64 {
    type Error = ManyError;

    fn try_into(self) -> Result<SubresourceId, Self::Error> {
        if self > (MAX_SUBRESOURCE_ID as u64) {
            Err(ManyError::invalid_identity_subid())
        } else {
            Ok(SubresourceId(self as u32))
        }
    }
}

impl From<SubresourceId> for u32 {
    fn from(id: SubresourceId) -> Self {
        id.0
    }
}

/// An identity address in the ManyVerse. This could be a server, network, user, DAO,
/// automated process, etc.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[must_use]
pub struct Address(InnerAddress);

impl Address {
    pub const ANONYMOUS: Self = Self::anonymous();
    pub const ILLEGAL: Self = Self::illegal();

    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ManyError> {
        InnerAddress::try_from(bytes).map(Self)
    }

    #[inline]
    pub const fn anonymous() -> Self {
        Self(InnerAddress::anonymous())
    }

    #[inline]
    pub const fn illegal() -> Self {
        Self(InnerAddress::illegal())
    }

    #[inline]
    pub const fn is_anonymous(&self) -> bool {
        self.0.is_anonymous()
    }

    #[inline]
    pub const fn is_illegal(&self) -> bool {
        self.0.is_illegal()
    }

    #[inline]
    pub const fn is_public_key(&self) -> bool {
        self.0.is_public_key()
    }

    #[inline]
    pub const fn is_subresource(&self) -> bool {
        self.0.is_subresource()
    }

    #[inline]
    pub const fn subresource_id(&self) -> Option<u32> {
        self.0.subresource_id()
    }

    #[inline]
    pub fn with_subresource_id<I: TryInto<SubresourceId, Error = ManyError>>(
        &self,
        subid: I,
    ) -> Result<Self, ManyError> {
        if let Some(h) = self.0.hash() {
            Ok(Self(InnerAddress::subresource_unchecked(
                h,
                subid.try_into()?,
            )))
        } else {
            Err(ManyError::invalid_identity_kind(self.0.bytes[0]))
        }
    }

    #[inline]
    pub fn into_public_key(self) -> Result<Self, ManyError> {
        if let Some(h) = self.0.hash() {
            Ok(Self::public_key_unchecked(h))
        } else {
            Err(ManyError::invalid_identity_kind(self.0.bytes[0]))
        }
    }

    #[inline]
    pub fn public_key(&self) -> Result<Self, ManyError> {
        (*self).into_public_key()
    }

    #[inline]
    pub const fn can_sign(&self) -> bool {
        self.is_public_key() || self.is_subresource()
    }

    #[inline]
    pub const fn can_be_source(&self) -> bool {
        self.is_anonymous() || self.is_public_key() || self.is_subresource()
    }

    #[inline]
    pub const fn can_be_dest(&self) -> bool {
        self.is_public_key() || self.is_subresource() || self.is_illegal()
    }

    #[inline]
    pub fn to_vec(self) -> Vec<u8> {
        self.0.to_vec()
    }

    #[inline]
    pub fn to_byte_array(self) -> [u8; MAX_IDENTITY_BYTE_LEN] {
        self.0.to_byte_array()
    }

    /// Check that another identity matches this one, ignoring any subresouce IDs.
    #[inline]
    pub fn matches(&self, other: &Address) -> bool {
        if self.is_anonymous() {
            other.is_anonymous()
        } else if self.is_illegal() {
            other.is_illegal()
        } else {
            // Extract public key hash of both.
            self.0.hash() == other.0.hash()
        }
    }

    /// Create an identity from the raw value of a public key hash, without checking
    /// its validity.
    ///
    /// This is unchecked to make sure the caller knows they are not supposed
    /// to use this function directly without thinking a bit more about it.
    ///
    /// Instead, use a utility function available in a separate crate (like
    /// many-identity-dsa) or in the testing utilities available here to create
    /// a bogus address.
    #[inline(always)]
    pub(crate) const fn public_key_unchecked(hash: PublicKeyHash) -> Self {
        Self(InnerAddress::public_key(hash))
    }
}

impl PartialEq<&str> for Address {
    #[allow(clippy::cmp_owned)]
    fn eq(&self, other: &&str) -> bool {
        self.to_string() == *other
    }
}

impl PartialEq<Option<Address>> for Address {
    fn eq(&self, other: &Option<Address>) -> bool {
        match other {
            Some(o) => o == self,
            None => self.is_anonymous(),
        }
    }
}

impl PartialEq<Address> for Option<Address> {
    fn eq(&self, other: &Address) -> bool {
        match self {
            Some(s) => other == s,
            None => other.is_anonymous(),
        }
    }
}

impl Debug for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Identity")
            .field(&if self.is_anonymous() {
                "anonymous".to_string()
            } else if self.is_illegal() {
                "illegal".to_string()
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

impl Default for Address {
    fn default() -> Self {
        Address::anonymous()
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.to_string())
    }
}

impl TryFrom<&[u8]> for Address {
    type Error = ManyError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl TryFrom<String> for Address {
    type Error = ManyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        InnerAddress::try_from(value.as_str()).map(Self)
    }
}

impl TryFrom<&str> for Address {
    type Error = ManyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        InnerAddress::try_from(value).map(Self)
    }
}

impl FromStr for Address {
    type Err = ManyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        InnerAddress::from_str(s).map(Self)
    }
}

impl AsRef<[u8; MAX_IDENTITY_BYTE_LEN]> for Address {
    fn as_ref(&self) -> &[u8; MAX_IDENTITY_BYTE_LEN] {
        let result: &[u8; MAX_IDENTITY_BYTE_LEN] = unsafe { std::mem::transmute(self) };
        result
    }
}

#[derive(Copy, Clone, Eq, Debug, Ord, PartialOrd)]
#[non_exhaustive]
#[must_use]
struct InnerAddress {
    bytes: [u8; MAX_IDENTITY_BYTE_LEN],
}

const DISCRIMINANT_ANONYMOUS: u8 = 0;
const DISCRIMINANT_PUBLIC_KEY: u8 = 1;
const DISCRIMINANT_ILLEGAL: u8 = 2;
// DISCRIMINANT_SUBRESOURCE_ID is useless here as it cannot be used in pattern matching.

// Identity needs to be bound to 32 bytes maximum.
static_assertions::assert_eq_size!([u8; MAX_IDENTITY_BYTE_LEN], InnerAddress);
const _1: () = assert!(InnerAddress::anonymous().to_byte_array()[0] == DISCRIMINANT_ANONYMOUS);
const _2: () = assert!(InnerAddress::illegal().to_byte_array()[0] == DISCRIMINANT_ILLEGAL);

impl PartialEq for InnerAddress {
    fn eq(&self, other: &Self) -> bool {
        match (self.bytes[0], other.bytes[0]) {
            // Anonymous
            (DISCRIMINANT_ANONYMOUS, DISCRIMINANT_ANONYMOUS) => true,

            // Public Key
            (DISCRIMINANT_PUBLIC_KEY, DISCRIMINANT_PUBLIC_KEY) => {
                self.bytes[1..=SHA_OUTPUT_SIZE] == other.bytes[1..=SHA_OUTPUT_SIZE]
            }

            // Illegal
            (DISCRIMINANT_ILLEGAL, DISCRIMINANT_ILLEGAL) => true,

            // Subresource.
            (x @ 0x80..=0xFF, y @ 0x80..=0xFF) if x == y => self.bytes[1..] == other.bytes[1..],

            // Anything else is by default inequal.
            (_, _) => false,
        }
    }
}

impl Default for InnerAddress {
    fn default() -> Self {
        InnerAddress::anonymous()
    }
}

impl InnerAddress {
    pub const fn anonymous() -> Self {
        Self {
            bytes: [DISCRIMINANT_ANONYMOUS; MAX_IDENTITY_BYTE_LEN],
        }
    }

    pub const fn illegal() -> Self {
        Self {
            bytes: [DISCRIMINANT_ILLEGAL; MAX_IDENTITY_BYTE_LEN],
        }
    }

    pub const fn public_key(hash: [u8; SHA_OUTPUT_SIZE]) -> Self {
        let mut bytes = [0; MAX_IDENTITY_BYTE_LEN];
        bytes[0] = DISCRIMINANT_PUBLIC_KEY;
        let mut len = SHA_OUTPUT_SIZE;
        while len > 0 {
            len -= 1;
            bytes[1 + len] = hash[len];
        }
        Self { bytes }
    }

    pub(crate) const fn subresource_unchecked(
        hash: [u8; SHA_OUTPUT_SIZE],
        id: SubresourceId,
    ) -> Self {
        // Get a public key and add the resource id.
        let mut bytes = Self::public_key(hash).bytes;
        bytes[0] = id.discriminant();

        let id = id.0;
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
            DISCRIMINANT_ANONYMOUS => {
                if bytes.len() > 1 {
                    Err(ManyError::invalid_identity())
                } else {
                    Ok(Self::anonymous())
                }
            }
            DISCRIMINANT_PUBLIC_KEY => {
                if bytes.len() != 29 {
                    Err(ManyError::invalid_identity())
                } else {
                    let mut slice = [0; 28];
                    slice.copy_from_slice(&bytes[1..29]);
                    Ok(Self::public_key(slice))
                }
            }
            DISCRIMINANT_ILLEGAL => {
                if bytes.len() > 1 {
                    Err(ManyError::invalid_identity())
                } else {
                    Ok(Self::illegal())
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
                    Ok(Self::subresource_unchecked(
                        hash,
                        SubresourceId(u32::from_be_bytes(subid)),
                    ))
                }
            }
            x => Err(ManyError::invalid_identity_kind(x.to_string())),
        }
    }

    pub fn from_str(value: &str) -> Result<Self, ManyError> {
        if !value.starts_with('m') {
            return Err(ManyError::invalid_identity_prefix(value[0..0].to_string()));
        }

        // Prevent subtract with overflow in the next block
        if value.len() < 3 {
            return Err(ManyError::invalid_identity());
        }

        if &value[1..] == "aa" || &value[1..] == "aaaa" {
            Ok(Self::anonymous())
        } else {
            let data = &value[..value.len() - 2][1..];
            let data = base32::decode(base32::Alphabet::RFC4648 { padding: false }, data)
                .ok_or_else(ManyError::invalid_identity)?;
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
            DISCRIMINANT_ANONYMOUS => vec![DISCRIMINANT_ANONYMOUS],
            DISCRIMINANT_PUBLIC_KEY => {
                let pk = &self.bytes[1..=SHA_OUTPUT_SIZE];
                vec![
                    DISCRIMINANT_PUBLIC_KEY,
                    pk[ 0], pk[ 1], pk[ 2], pk[ 3], pk[ 4], pk[ 5], pk[ 6], pk[ 7],
                    pk[ 8], pk[ 9], pk[10], pk[11], pk[12], pk[13], pk[14], pk[15],
                    pk[16], pk[17], pk[18], pk[19], pk[20], pk[21], pk[22], pk[23],
                    pk[24], pk[25], pk[26], pk[27],
                ]
            }
            DISCRIMINANT_ILLEGAL => vec![DISCRIMINANT_ILLEGAL],
            0x80..=0xFF => {
                self.bytes.to_vec()
            }
            _ => unreachable!(),
        }
    }

    pub const fn is_anonymous(&self) -> bool {
        self.bytes[0] == DISCRIMINANT_ANONYMOUS
    }
    pub const fn is_illegal(&self) -> bool {
        self.bytes[0] == DISCRIMINANT_ILLEGAL
    }
    pub const fn is_public_key(&self) -> bool {
        self.bytes[0] == DISCRIMINANT_PUBLIC_KEY
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

impl std::fmt::Display for InnerAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_anonymous() {
            // Special case this.
            return write!(f, "maa");
        }

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

impl TryFrom<&str> for InnerAddress {
    type Error = ManyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        InnerAddress::from_str(value)
    }
}

impl TryFrom<&[u8]> for InnerAddress {
    type Error = ManyError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

#[cfg(test)]
pub mod tests {
    use super::DISCRIMINANT_ANONYMOUS;
    use crate::testing::identity;
    use crate::Address;
    use serde_test::{assert_tokens, Configure, Token};
    use std::str::FromStr;

    #[test]
    fn can_read_anonymous() {
        let a = Address::anonymous();
        let a_str = a.to_string();
        let a2 = Address::from_str(&a_str).unwrap();

        assert_eq!(a, a2);
    }

    #[test]
    fn can_read_anonymous_short() {
        assert_eq!(Address::from_str("maa"), Ok(Address::anonymous()));
    }

    #[test]
    fn eq() {
        assert_eq!(Address::from_str("maa").unwrap(), "maa");
        assert_eq!(
            Address::from_str("mahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqys").unwrap(),
            "mahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqys"
        );
        assert_eq!(Address::illegal(), "maiyg");
    }

    #[test]
    fn byte_array_conversion() {
        let a = Address::anonymous();
        let b = identity(1);
        let c = identity(2);

        assert_ne!(a.to_string(), b.to_string());
        assert_ne!(b.to_string(), c.to_string());
        assert_ne!(a.to_vec(), b.to_vec());
        assert_ne!(b.to_vec(), c.to_vec());

        assert_eq!(Address::from_str(&a.to_string()), Ok(a));
        assert_eq!(Address::from_str(&b.to_string()), Ok(b));
        assert_eq!(Address::from_str(&c.to_string()), Ok(c));
    }

    #[test]
    fn into_public_key() {
        let a =
            Address::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap();

        assert!(!a.is_public_key());
        let b = a.into_public_key().unwrap();
        assert!(b.is_public_key());
        assert!(Address::anonymous().into_public_key().is_err());
        assert!(Address::illegal().into_public_key().is_err());
    }

    #[test]
    fn textual_format_1() {
        let a = Address::from_str("mahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqys").unwrap();
        let b = Address::from_bytes(
            &hex::decode("01c8aead03f915f128f0fa7ff696c656eaa93db87bd9aa73df693acb22").unwrap(),
        )
        .unwrap();

        assert!(a.is_public_key());
        assert!(b.is_public_key());
        assert_eq!(a, b);
    }

    #[test]
    fn textual_format_2() {
        let a =
            Address::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap();
        let b = Address::from_bytes(
            &hex::decode("804a101d521d810211a0c6346ba89bd1cc1f821c03b969ff9d5c8b2f59000001")
                .unwrap(),
        )
        .unwrap();

        assert!(a.is_subresource());
        assert!(b.is_subresource());
        assert_eq!(a, b);
    }

    #[test]
    fn neq_textual_format_1() {
        let a = Address::from_str("mahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqys").unwrap();
        let b =
            Address::from_str("mqdek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqaaaaqim").unwrap();

        assert_ne!(a, b);
        assert_eq!(a, b.into_public_key().unwrap());
    }

    #[test]
    fn textual_format_anonymous() {
        let a = Address::from_str("maa").unwrap();
        assert!(a.is_anonymous());
        let b = Address::from_str("maaaa").unwrap();
        assert!(b.is_anonymous());
        let c = Address::from_bytes(&hex::decode("00").unwrap()).unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn textual_format_illegal() {
        let a = Address::from_str("maiyg").unwrap();
        assert!(a.is_illegal());
        let b = Address::from_bytes(&hex::decode("02").unwrap()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn subresource_1() {
        let a = Address::from_str("mahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqys")
            .unwrap()
            .with_subresource_id(1)
            .unwrap();
        let b = Address::from_bytes(
            &hex::decode("80c8aead03f915f128f0fa7ff696c656eaa93db87bd9aa73df693acb22000001")
                .unwrap(),
        )
        .unwrap();
        let c = Address::from_bytes(
            &hex::decode("80c8aead03f915f128f0fa7ff696c656eaa93db87bd9aa73df693acb22000002")
                .unwrap(),
        )
        .unwrap();

        assert!(a.is_subresource());
        assert!(b.is_subresource());
        assert!(c.is_subresource());

        assert_eq!(a, b);
        assert_eq!(b.with_subresource_id(2).unwrap(), c);

        assert_eq!(a.subresource_id(), Some(1));
        assert_eq!(b.subresource_id(), Some(1));
        assert_eq!(c.subresource_id(), Some(2));
    }

    #[test]
    fn subresource_id() {
        let a = Address::from_bytes(
            &hex::decode("81c8aead03f915f128f0fa7ff696c656eaa93db87bd9aa73df693acb22234567")
                .unwrap(),
        )
        .unwrap();

        assert_eq!(a.subresource_id(), Some(0x01234567));
        assert_eq!(a.public_key().unwrap().subresource_id(), None);
        assert_eq!(Address::anonymous().subresource_id(), None);
        assert_eq!(Address::illegal().subresource_id(), None);
    }

    #[test]
    fn matches() {
        let a =
            Address::from_str("mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz").unwrap();
        let b = a.public_key().unwrap();

        assert_ne!(a, b);
        assert!(a.matches(&b));
        assert!(b.matches(&a));

        assert!(!Address::anonymous().matches(&a));
        assert!(!Address::illegal().matches(&a));
    }

    proptest::proptest! {
        #[test]
        fn subresource_id_fuzzy(subid: u32) {
            let a = Address::from_str("mahek5lid7ek7ckhq7j77nfwgk3vkspnyppm2u467ne5mwiqys")
                .unwrap()
                .with_subresource_id(subid);

            if let Ok(id) = a {
                let b = Address::from_str(&id.to_string());
                assert_eq!(a, b);
            } else {
                assert_eq!(subid.leading_zeros(), 0);
            }
        }
    }

    #[test]
    fn serde_anonymous() {
        let id = Address::anonymous();
        assert_tokens(&id.readable(), &[Token::String("maa")]);
        assert_tokens(&id.compact(), &[Token::Bytes(&[DISCRIMINANT_ANONYMOUS])]);
    }

    #[test]
    fn from_str_overflow() {
        assert!(Address::from_str("m").is_err());
        assert!(Address::from_str("ma").is_err());
        assert!(Address::from_str("maa").is_ok());
        assert!(Address::from_str("maaa").is_err());
        assert!(Address::from_str("maaaa").is_ok());
        assert!(Address::from_str(
            "maa010203040506070809101112131415161718192021222324252627282930"
        )
        .is_err());
    }
}
