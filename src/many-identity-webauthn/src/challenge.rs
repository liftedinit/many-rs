use coset::{CoseSign1, ProtectedHeader};
use many_error::ManyError;
use many_types::cbor::Base64Encoder;
use minicbor::{Decode, Encode};
use sha2::{Digest, Sha512};

mod protected_header_cbor {
    use coset::cbor::value::Value;
    use coset::{CborSerializable, ProtectedHeader};
    use minicbor::encode::Write;
    use minicbor::{Decoder, Encoder};

    pub fn encode<C, W: Write>(
        value: &ProtectedHeader,
        e: &mut Encoder<W>,
        _: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        let v = CborSerializable::to_vec(value.clone())
            .map_err(|e| minicbor::encode::Error::message(format!("Cose error: {e}")))?;

        e.bytes(&v)?;
        Ok(())
    }

    pub fn decode<C>(
        d: &mut Decoder<'_>,
        _: &mut C,
    ) -> Result<ProtectedHeader, minicbor::decode::Error> {
        let bytes = d.bytes()?;
        ProtectedHeader::from_cbor_bstr(Value::Bytes(bytes.to_vec()))
            .map_err(|e| minicbor::decode::Error::message(format!("Cose error: {e}")))
    }
}

/// A WebAuthn challenge for the MANY protocol.
#[derive(Clone, Encode, Decode)]
#[cbor(map)]
pub struct Challenge {
    #[cbor(n(0), with = "protected_header_cbor")]
    protected_header: ProtectedHeader,

    #[n(1)]
    payload_sha: Base64Encoder<Vec<u8>>,
}

impl Challenge {
    pub fn payload_sha(&self) -> &[u8] {
        &self.payload_sha.0
    }

    pub fn protected_header(&self) -> &ProtectedHeader {
        &self.protected_header
    }
}

impl TryInto<Challenge> for &CoseSign1 {
    type Error = ManyError;

    fn try_into(self) -> Result<Challenge, Self::Error> {
        let protected_header = self.protected.clone();

        let mut hash = Sha512::new();
        if let Some(payload) = &self.payload {
            hash.update(payload);
        }

        let payload_sha = hash.finalize().to_vec().into();

        Ok(Challenge {
            protected_header,
            payload_sha,
        })
    }
}
