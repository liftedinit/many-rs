use coset::{CoseSign1, TaggedCborSerializable};
use many_error::ManyError;
use many_identity::{Address, Verifier};
use many_macros::many_module;
use many_protocol::{IdentityResolver, RequestMessage, ResponseMessage};
use many_types::attributes::TryFromAttributeSet;
use minicbor::encode::{Error, Write};
use minicbor::{decode, encode, Decode, Decoder, Encode, Encoder};

pub mod attributes {
    use coset::{CoseSign1, TaggedCborSerializable};
    use many_error::ManyError;
    use many_identity::{Address, Verifier};
    use many_types::attributes::{Attribute, AttributeSet, TryFromAttributeSet};
    use many_types::cbor::CborAny;
    use many_types::Timestamp;

    /// Same ID for Request and Response.
    pub const DELEGATION: Attribute = Attribute::id(2);

    pub struct DelegationAttribute {
        inner: Vec<CoseSign1>,
    }

    impl DelegationAttribute {
        pub fn new(inner: Vec<CoseSign1>) -> Self {
            Self { inner }
        }

        fn create_from_bytes(bytes: Vec<u8>) -> Result<Self, ManyError> {
            let cose_sign_1 =
                CoseSign1::from_tagged_slice(&bytes).map_err(ManyError::deserialization_error)?;
            Ok(Self {
                inner: vec![cose_sign_1],
            })
        }

        fn create_from_array(array: Vec<CborAny>) -> Result<Self, ManyError> {
            Ok(Self {
                inner: array
                    .into_iter()
                    .map(|any| match any {
                        CborAny::Bytes(bytes) => CoseSign1::from_tagged_slice(&bytes)
                            .map_err(ManyError::deserialization_error),
                        _ => Err(ManyError::invalid_attribute_arguments()),
                    })
                    .collect::<Result<Vec<CoseSign1>, _>>()?,
            })
        }

        /// Resolve and validate a series of certificates, returning the last delegation address.
        pub fn resolve(
            &self,
            to: Address,
            verifier: &impl Verifier,
            now: Timestamp,
        ) -> Result<Address, ManyError> {
            let mut current_address = to;
            let mut it = self.inner.iter().peekable();
            while let Some(envelope) = it.next() {
                let is_last = it.peek().is_none();
                let cert = many_types::delegation::Certificate::decode_and_verify(
                    envelope, verifier, now, is_last,
                )?;
                if cert.to == current_address {
                    current_address = cert.from;
                } else {
                    return Err(ManyError::unknown("Invalid certificate."));
                }
            }

            if !self.inner.is_empty() {
                tracing::trace!("Resolved delegation from {} to {}", to, current_address);
            }

            Ok(current_address)
        }
    }

    impl TryInto<Attribute> for DelegationAttribute {
        type Error = ManyError;

        fn try_into(self) -> Result<Attribute, Self::Error> {
            Ok(DELEGATION.with_argument(if self.inner.len() == 1 {
                CborAny::Bytes(
                    self.inner
                        .into_iter()
                        .next()
                        .unwrap()
                        .to_tagged_vec()
                        .map_err(ManyError::serialization_error)?,
                )
            } else {
                CborAny::Array(
                    self.inner
                        .into_iter()
                        .map(|s| {
                            s.to_tagged_vec()
                                .map_err(ManyError::serialization_error)
                                .map(CborAny::Bytes)
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            }))
        }
    }

    impl TryFrom<Attribute> for DelegationAttribute {
        type Error = ManyError;

        fn try_from(value: Attribute) -> Result<Self, Self::Error> {
            if value.id != DELEGATION.id {
                return Err(ManyError::invalid_attribute_id(value.id));
            }

            let arguments = value.into_arguments();
            if arguments.len() != 1 {
                Err(ManyError::invalid_attribute_arguments())
            } else {
                match arguments.into_iter().next() {
                    Some(CborAny::Bytes(inner)) => Self::create_from_bytes(inner),
                    Some(CborAny::Array(inner)) => Self::create_from_array(inner),
                    _ => Err(ManyError::invalid_attribute_arguments()),
                }
            }
        }
    }

    impl TryFromAttributeSet for DelegationAttribute {
        fn try_from_set(set: &AttributeSet) -> Result<Self, ManyError> {
            match set.get_attribute(DELEGATION.id) {
                Some(attr) => Self::try_from(attr.clone()),
                None => Err(ManyError::attribute_not_found(DELEGATION.id)),
            }
        }
    }
}

pub struct DelegationResolver<V: Verifier> {
    inner: V,
}

impl<V: Verifier + Clone> Clone for DelegationResolver<V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<V: Verifier> DelegationResolver<V> {
    pub fn new(verifier: V) -> Self {
        Self { inner: verifier }
    }
}

impl<V: Verifier> IdentityResolver for DelegationResolver<V> {
    fn resolve_request(
        &self,
        request: &RequestMessage,
        from: Address,
    ) -> Result<Address, ManyError> {
        if let Ok(attr) = attributes::DelegationAttribute::try_from_set(&request.attributes) {
            let timestamp = request
                .timestamp
                .ok_or_else(|| ManyError::unknown("Request must have a timestamp."))?;
            attr.resolve(from, &self.inner, timestamp)
        } else {
            Ok(from)
        }
    }

    fn resolve_response(
        &self,
        response: &ResponseMessage,
        from: Address,
    ) -> Result<Address, ManyError> {
        if let Ok(attr) = attributes::DelegationAttribute::try_from_set(&response.attributes) {
            let timestamp = response
                .timestamp
                .ok_or_else(|| ManyError::unknown("Response must have a timestamp."))?;
            attr.resolve(from, &self.inner, timestamp)
        } else {
            Ok(from)
        }
    }
}

#[derive(Debug)]
pub struct CreateCertificateReturn {
    pub certificate: CoseSign1,
}

impl<C> Encode<C> for CreateCertificateReturn {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _ctx: &mut C) -> Result<(), Error<W::Error>> {
        e.map(1)?.i8(0)?.bytes(
            &self
                .certificate
                .clone()
                .to_tagged_vec()
                .map_err(|_| encode::Error::message("Could not serialize certificate"))?,
        )?;
        Ok(())
    }
}
impl<'b, C> Decode<'b, C> for CreateCertificateReturn {
    fn decode(d: &mut Decoder<'b>, _ctx: &mut C) -> Result<Self, decode::Error> {
        if d.map()? != Some(1) {
            return Err(decode::Error::message("Need one item"));
        }

        let bytes = d.bytes()?;
        let cose_sign_1 = CoseSign1::from_tagged_slice(bytes)
            .map_err(|e| decode::Error::message(e.to_string()))?;
        Ok(Self {
            certificate: cose_sign_1,
        })
    }
}

#[derive(Debug, Clone, Encode, Decode)]
#[cbor(map)]
pub struct WhoAmIReturn {
    #[n(0)]
    pub address: Address,
}

#[many_module(name = DelegationModule, namespace = "delegation", id = 10, many_modules_crate = crate)]
pub trait DelegationModuleBackend: Send {
    fn create_certificate(&self) -> Result<CreateCertificateReturn, ManyError> {
        Err(ManyError::unknown("Server does not support delegation."))
    }

    #[many(no_payload)]
    fn who_am_i(&self, sender: &Address) -> Result<WhoAmIReturn, ManyError> {
        Ok(WhoAmIReturn { address: *sender })
    }
}
