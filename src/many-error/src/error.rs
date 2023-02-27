use crate::Reason;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::iter::FromIterator;

#[cfg(feature = "minicbor")]
mod minicbor;

macro_rules! many_error {
    {
        $(
            $v: literal: $name: ident $(as $snake_name: ident ( $($arg: ident),* ))? => $description: literal,
        )*
    } => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
        pub enum ManyErrorCode {
            $( $name, )*
            AttributeSpecific(i32),
            ApplicationSpecific(u32),
        }

        impl ManyErrorCode {
            #[inline]
            pub fn message(&self) -> Option<&'static str> {
                match self {
                    $( ManyErrorCode::$name => Some($description), )*
                    _ => None,
                }
            }
        }

        impl Display for ManyErrorCode {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                match self.message() {
                    Some(msg) => f.write_str(msg),
                    None => write!(f, "{}", Into::<i64>::into(*self)),
                }
            }
        }

        impl From<i64> for ManyErrorCode {
            fn from(v: i64) -> Self {
                match v {
                    $(
                        $v => Self::$name,
                    )*
                    x if x >= 0 => Self::ApplicationSpecific(x as u32),
                    x if x <= -10000 => Self::AttributeSpecific(x as i32),
                    _ => Self::Unknown,
                }
            }
        }
        impl From<ManyErrorCode> for i64 {
            fn from(v: ManyErrorCode) -> i64 {
                match v {
                    $(
                        ManyErrorCode::$name => $v,
                    )*
                    ManyErrorCode::AttributeSpecific(x) => x as i64,
                    ManyErrorCode::ApplicationSpecific(x) => x as i64,
                }
            }
        }

        impl ManyError {
            $($(
                #[doc = $description]
                pub fn $snake_name( $($arg: impl ToString,)* ) -> Self {
                    let s = Self::new(
                        ManyErrorCode::$name,
                        Some($description.to_string()),
                        BTreeMap::from_iter(vec![
                            $( (stringify!($arg).to_string(), ($arg).to_string()) ),*
                        ]),
                    );

                    #[cfg(feature = "trace_error_creation")] {
                        tracing::trace!("{}", s);
                        tracing::trace!("Backtrace:\n{:?}", backtrace::Backtrace::new());
                    }

                    s
                }
            )?)*
        }
    }
}

many_error! {
    // Range -0 - -999 is for generic, unexpected or transport errors.
       -1: Unknown as unknown(message)
            => "Unknown error: {message}",
       -2: MessageTooLong as message_too_long(max)
            => "Message is too long. Max allowed size is {max} bytes.",
       -3: DeserializationError as deserialization_error(details)
            => "Deserialization error:\n{details}",
       -4: SerializationError as serialization_error(details)
            => "Serialization error:\n{details}",
       -5: UnexpectedEmptyRequest as unexpected_empty_request()
            => "Request of a message was unexpectedly empty.",
       -6: UnexpectedEmptyResponse as unexpected_empty_response()
            => "Response of a message was unexpectedly empty.",
       -7: UnexpectedTransportError as unexpected_transport_error(inner)
            => "The transport returned an error unexpectedly:\n{inner}",
       -8: CouldNotRouteMessage as could_not_route_message()
            => "Could not find a handler for the message.",
       -9: InvalidAttribtueId as invalid_attribute_id(id) => "Unexpected attribute ID: {id}.",
      -10: InvalidAttributeArguments as invalid_attribute_arguments()
            => "Attribute does not have the right arguments.",
      -11: AttributeNotFound as attribute_not_found(id) => "Expected attribute {id} not found.",

     -100: InvalidIdentity as invalid_identity()
            => "Identity is invalid (does not follow the protocol).",
     -101: InvalidIdentityPrefix as invalid_identity_prefix(actual)
            => "Identity string did not start with the right prefix. Expected 'm', was '{actual}'.",
     -102: InvalidIdentityKind as invalid_identity_kind(actual)
            => r#"Identity kind "{actual}" was not recognized."#,
     -103: InvalidIdentitySubResourceId as invalid_identity_subid()
            => "Invalid Subresource ID. Subresource IDs are 31 bits.",
     -104: SenderCannotBeAnonymous as sender_cannot_be_anonymous()
            => "Invalid Identity; the sender cannot be anonymous.",

     // HSM-related errors
     -200: HSMInitError as hsm_init_error(details)
            => "PKCS#11 init error:\n{details}",
     -201: HSMSessionError as hsm_session_error(details)
            => "PKCS#11 session error:\n{details}",
     -202: HSMLoginError as hsm_login_error(details)
            => "PKCS#11 login error:\n{details}",
     -203: HSMKeyIdError as hsm_keyid_error(details)
            => "PKCS#11 key ID error:\n{details}",
     -204: HSMSignError as hsm_sign_error(details)
            => "PKCS#11 sign error:\n{details}",
     -205: HSMVerifyError as hsm_verify_error(details)
            => "PKCS#11 verify error:\n{details}",
     -206: HSMECPointError as hsm_ec_point_error(details)
            => "PKCS#11 EC Point error:\n{details}",
     -207: HSMECParamsError as hsm_ec_params_error(details)
            => "PKCS#11 EC Params error:\n{details}",
     -208: HSMKeygenError as hsm_keygen_error(details)
            => "PKCS#11 key generation error:\n{details}",
     -209: HSMMutexPoisoned as hsm_mutex_poisoned(details)
            => "PKCS#11 global instance mutex poisoned:\n{details}",

    // -1000 - -1999 is for request errors.
    -1000: InvalidMethodName as invalid_method_name(method)
            => r#"Invalid method name: "{method}"."#,
    -1001: InvalidFromIdentity as invalid_from_identity()
            => "The identity of the from field is invalid or unexpected.",
    -1002: InvalidToIdentity as invalid_to_identity()
            => "The identity of the to field is invalid or unexpected.",
    -1003: CouldNotVerifySignature as could_not_verify_signature(details)
            => "Could not verify the signature: {details}.",
    -1004: UnknownDestination as unknown_destination(to, this)
            => "Unknown destination for message.\nThis is \"{this}\", message was for \"{to}\".",
    -1005: EmptyEnvelope as empty_envelope()
            => "An envelope must contain a payload.",
    -1006: TimestampOutOfRange as timestamp_out_of_range()
            => "The message's timestamp is out of the accepted range of the server.",
    -1007: RequiredFieldMissing as required_field_missing(field)
            => "Field is required but missing: '{field}'.",
    -1008: NonWebAuthnRequestDenied as non_webauthn_request_denied(endpoint)
            => "Non-WebAuthn request denied for endpoint '{endpoint}'.",

    // -2000 - -2999 is for server errors.
    -2000: InternalServerError as internal_server_error()
            => "An internal server error happened.",

    // Negative 10000+ are reserved for attribute specified codes and are defined separately.
    // The method to use these is ATTRIBUTE_ID * -10000.

    // Positive error codes are reserved for application specific errors and custom
    // server-specific error messages.
}

/// Easily define ManyError for specific attributes.
#[macro_export]
macro_rules! define_attribute_many_error {
    ( $( attribute $module_id: literal => { $( $id: literal : $vis: vis fn $name: ident ($( $var_name: ident ),*) => $message: literal ),* $(,)? } );* ) => {
        $(
        $(
            $vis fn $name( $($var_name: impl ToString),* ) -> $crate::ManyError {
                $crate::ManyError::attribute_specific(
                    ($module_id as i32) * -10000i32 - ($id as i32),
                    String::from($message),
                    std::iter::FromIterator::from_iter(vec![
                        $( (stringify!($var_name).to_string(), ($var_name).to_string()) ),*
                    ]),
                )
            }
        )*
        )*
    }
}
/// Easily define ManyError for specific application.
#[macro_export]
macro_rules! define_application_many_error {
    ( $( { $( $id: literal : $vis: vis fn $name: ident ($( $var_name: ident ),*) => $message: literal ),* $(,)? } );* ) => {
        $(
        $(
            $vis fn $name ( $($var_name: impl ToString),* ) -> $crate::ManyError {
                $crate::ManyError::application_specific(
                    $id as u32,
                    String::from($message),
                    std::iter::FromIterator::from_iter(vec![
                        $( (stringify!($var_name).to_string(), ($var_name).to_string()) ),*
                    ]),
                )
            }
        )*
        )*
    }
}

impl ManyErrorCode {
    #[inline]
    pub const fn is_attribute_specific(&self) -> bool {
        matches!(self, ManyErrorCode::AttributeSpecific(_))
    }
    #[inline]
    pub const fn is_application_specific(&self) -> bool {
        matches!(self, ManyErrorCode::ApplicationSpecific(_))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct ManyError(Reason<ManyErrorCode>);

impl ManyError {
    #[inline]
    pub const fn code(&self) -> ManyErrorCode {
        *self.0.code()
    }

    #[inline]
    pub fn message(&self) -> Option<&str> {
        self.0.message()
    }

    #[inline]
    pub fn set_message(&mut self, message: Option<String>) {
        self.0.set_message(message)
    }

    #[inline]
    pub fn argument<S: AsRef<str>>(&self, field: S) -> Option<&str> {
        self.0.argument(field)
    }

    #[inline]
    pub fn arguments(&self) -> &BTreeMap<String, String> {
        self.0.arguments()
    }

    #[inline]
    pub fn add_argument(&mut self, key: String, value: String) {
        self.0.add_argument(key, value);
    }

    #[inline]
    pub const fn is_attribute_specific(&self) -> bool {
        self.code().is_attribute_specific()
    }

    #[inline]
    pub const fn is_application_specific(&self) -> bool {
        self.code().is_application_specific()
    }

    pub const fn new(
        code: ManyErrorCode,
        message: Option<String>,
        arguments: BTreeMap<String, String>,
    ) -> Self {
        Self(Reason::new(code, message, arguments))
    }

    pub fn with_code(self, code: ManyErrorCode) -> Self {
        Self(self.0.with_code(code))
    }

    #[inline]
    pub const fn attribute_specific(
        code: i32,
        message: String,
        arguments: BTreeMap<String, String>,
    ) -> Self {
        Self::new(
            ManyErrorCode::AttributeSpecific(code),
            Some(message),
            arguments,
        )
    }

    #[inline]
    pub const fn application_specific(
        code: u32,
        message: String,
        arguments: BTreeMap<String, String>,
    ) -> Self {
        Self::new(
            ManyErrorCode::ApplicationSpecific(code),
            Some(message),
            arguments,
        )
    }
}

impl Display for ManyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// TODO: The fact that ManyErrorCode is constructed as a macro makes this annoying, instead of just deriving Default.
#[allow(clippy::derivable_impls)]
impl Default for ManyErrorCode {
    #[inline]
    fn default() -> Self {
        ManyErrorCode::Unknown
    }
}

impl std::error::Error for ManyError {}

impl Default for ManyError {
    #[inline]
    fn default() -> Self {
        ManyError::unknown("?")
    }
}

#[cfg(test)]
mod tests {
    use super::ManyError;
    use crate::ManyErrorCode as ErrorCode;
    use std::collections::BTreeMap;

    #[test]
    fn works() {
        let mut arguments = BTreeMap::new();
        arguments.insert("0".to_string(), "ZERO".to_string());
        arguments.insert("1".to_string(), "ONE".to_string());
        arguments.insert("2".to_string(), "TWO".to_string());

        let e = ManyError::new(
            ErrorCode::Unknown,
            Some("Hello {0} and {2}.".to_string()),
            arguments,
        );

        assert_eq!(e.to_string(), "Hello ZERO and TWO.");
    }

    #[test]
    fn works_with_only_replacement() {
        let mut arguments = BTreeMap::new();
        arguments.insert("0".to_string(), "ZERO".to_string());
        arguments.insert("1".to_string(), "ONE".to_string());
        arguments.insert("2".to_string(), "TWO".to_string());

        let e = ManyError::new(ErrorCode::Unknown, Some("{2}".to_string()), arguments);

        assert_eq!(e.to_string(), "TWO");
    }

    #[test]
    fn works_for_others() {
        let mut arguments = BTreeMap::new();
        arguments.insert("0".to_string(), "ZERO".to_string());
        arguments.insert("1".to_string(), "ONE".to_string());
        arguments.insert("2".to_string(), "TWO".to_string());

        let e = ManyError::new(
            ErrorCode::Unknown,
            Some("@{a}{b}{c}.".to_string()),
            arguments,
        );

        assert_eq!(e.to_string(), "@.");
    }

    #[test]
    fn supports_double_brackets() {
        let mut arguments = BTreeMap::new();
        arguments.insert("0".to_string(), "ZERO".to_string());
        arguments.insert("1".to_string(), "ONE".to_string());
        arguments.insert("2".to_string(), "TWO".to_string());

        let e = ManyError::new(
            ErrorCode::Unknown,
            Some("/{{}}{{{0}}}{{{a}}}{b}}}{{{2}.".to_string()),
            arguments,
        );

        assert_eq!(e.to_string(), "/{}{ZERO}{}}{TWO.");
    }
}
