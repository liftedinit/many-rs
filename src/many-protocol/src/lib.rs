use coset::CoseSign1;
use coset::CoseSign1Builder;
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};

pub mod request;
pub mod response;

pub use request::{RequestMessage, RequestMessageBuilder};
pub use response::{ResponseMessage, ResponseMessageBuilder};

pub type ManyUrl = reqwest::Url;

/// A resolver for identities that take a request or response and validates and returns a resolved
/// identity.
pub trait IdentityResolver: Send {
    fn resolve_request(
        &self,
        request: &RequestMessage,
        from: Address,
    ) -> Result<Address, ManyError>;
    fn resolve_response(
        &self,
        response: &ResponseMessage,
        from: Address,
    ) -> Result<Address, ManyError>;
}

macro_rules! decl_redirection {
    (
        $(
            fn $name: ident ( &self $(,)? $($argn: ident : $argt: ty),* $(,)? ) -> $ret: tt $( < $( $lt:tt ),+ > )?
        ),* $(,)?
    ) => {
        $(
        fn $name ( &self, $($argn : $argt),* ) -> $ret $(< $( $lt ),+ >)? {
            (&**self) . $name ( $($argn),* )
        }
        )*
    };
}

macro_rules! decl_resolver_impl {
    (
        $(
            impl $(
                < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+ >
            )? for $ty: ty
        );+ $(;)?
    ) => {
        $(
        impl $(< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? IdentityResolver for $ty {
            decl_redirection!(
                fn resolve_request(
                    &self,
                    request: &RequestMessage,
                    from: Address,
                ) -> Result<Address, ManyError>,
                fn resolve_response(
                    &self,
                    response: &ResponseMessage,
                    from: Address,
                ) -> Result<Address, ManyError>,
            );
        }
        )+
    };
}

decl_resolver_impl!(
    impl for Box<dyn IdentityResolver>;
    impl<I: IdentityResolver> for Box<I>;
    impl<I: IdentityResolver + Sync> for std::sync::Arc<I>;
);

macro_rules! declare_tuple_resolvers {
    ( $name: ident: 0 ) => {
        impl< $name: IdentityResolver > IdentityResolver for ( $name, ) {
            #[inline]
            fn resolve_request(
                &self,
                request: &RequestMessage,
                from: Address,
            ) -> Result<Address, ManyError> {
                self.0.resolve_request(request, from)
            }

            #[inline]
            fn resolve_response(
                &self,
                response: &ResponseMessage,
                from: Address,
            ) -> Result<Address, ManyError> {
                self.0.resolve_response(response, from)
            }
        }
    };

    ( $( $name: ident: $index: tt ),* ) => {
        impl< $( $name: IdentityResolver ),* > IdentityResolver for ( $( $name ),* ) {
            #[inline]
            fn resolve_request(
                &self,
                request: &RequestMessage,
                from: Address,
            ) -> Result<Address, ManyError> {
                $(
                    let from = self. $index . resolve_request(request, from)?;
                )*
                Ok(from)
            }

            #[inline]
            fn resolve_response(
                &self,
                response: &ResponseMessage,
                from: Address,
            ) -> Result<Address, ManyError> {
                $(
                    let from = self. $index . resolve_response(response, from)?;
                )*
                Ok(from)
            }
        }
    };
}

// 8 outta be enough for everyone (but you can also ((a, b), (c, d), ...) recursively).
declare_tuple_resolvers!(A: 0);
declare_tuple_resolvers!(A: 0, B: 1);
declare_tuple_resolvers!(A: 0, B: 1, C: 2);
declare_tuple_resolvers!(A: 0, B: 1, C: 2, D: 3);
declare_tuple_resolvers!(A: 0, B: 1, C: 2, D: 3, E: 4);
declare_tuple_resolvers!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);
declare_tuple_resolvers!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6);
declare_tuple_resolvers!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7);

#[derive(Clone)]
pub struct BaseIdentityResolver;

impl IdentityResolver for BaseIdentityResolver {
    fn resolve_request(
        &self,
        request: &RequestMessage,
        from: Address,
    ) -> Result<Address, ManyError> {
        if !from.matches(&request.from.unwrap_or_default()) {
            return Err(ManyError::invalid_from_identity());
        }
        Ok(request.from.unwrap_or_default())
    }

    fn resolve_response(
        &self,
        response: &ResponseMessage,
        from: Address,
    ) -> Result<Address, ManyError> {
        if !from.matches(&response.from) {
            return Err(ManyError::invalid_from_identity());
        }
        Ok(response.from)
    }
}

pub fn decode_request_from_cose_sign1(
    envelope: &CoseSign1,
    verifier: &impl Verifier,
    resolver: &impl IdentityResolver,
) -> Result<RequestMessage, ManyError> {
    let from_id = verifier.verify_1(envelope)?;

    let payload = envelope
        .payload
        .as_ref()
        .ok_or_else(ManyError::empty_envelope)?;
    let mut message =
        RequestMessage::from_bytes(payload).map_err(ManyError::deserialization_error)?;

    // Resolve and update the `from` identity. The resolution might not return the same identity
    // at all.
    let from_id = resolver.resolve_request(&message, from_id)?;
    message.from = Some(from_id);

    Ok(message)
}

pub fn decode_response_from_cose_sign1(
    envelope: &CoseSign1,
    to: Option<Address>,
    verifier: &impl Verifier,
    resolver: &impl IdentityResolver,
) -> Result<ResponseMessage, ManyError> {
    let message = ResponseMessage::decode_and_verify(envelope, verifier, resolver)?;

    // Check the `to` field to make sure we have the right one.
    if let Some(to_id) = to {
        if to_id != message.to.unwrap_or_default() {
            return Err(ManyError::invalid_to_identity());
        }
    }

    Ok(message)
}

fn encode_cose_sign1_from_payload(
    payload: Vec<u8>,
    identity: &impl Identity,
) -> Result<CoseSign1, ManyError> {
    let sign1 = CoseSign1Builder::default().payload(payload).build();
    identity.sign_1(sign1)
}

pub fn encode_cose_sign1_from_response(
    response: ResponseMessage,
    identity: &impl Identity,
) -> Result<CoseSign1, ManyError> {
    encode_cose_sign1_from_payload(response.to_bytes().unwrap(), identity)
}

pub fn encode_cose_sign1_from_request(
    request: RequestMessage,
    identity: &impl Identity,
) -> Result<CoseSign1, ManyError> {
    encode_cose_sign1_from_payload(request.to_bytes().unwrap(), identity)
}
