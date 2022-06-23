use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use num_derive::{FromPrimitive, ToPrimitive};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::iter::FromIterator;

#[derive(FromPrimitive, ToPrimitive)]
#[repr(i8)]
enum ReasonCborKey {
    Code = 0,
    Message = 1,
    Arguments = 2,
}

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

        impl Encode for ManyErrorCode {
            #[inline]
            fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
                e.i64((*self).into())?;
                Ok(())
            }
        }

        impl<'b> Decode<'b> for ManyErrorCode {
            fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
                Ok(d.i64()?.into())
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
    -1002: CouldNotVerifySignature as could_not_verify_signature(details)
            => "Could not verify the signature: {details}.",
    -1003: UnknownDestination as unknown_destination(to, this)
            => "Unknown destination for message.\nThis is \"{this}\", message was for \"{to}\".",
    -1004: EmptyEnvelope as empty_envelope()
            => "An envelope must contain a payload.",
    -1005: TimestampOutOfRange as timestamp_out_of_range()
            => "The message's timestamp is out of the accepted range of the server.",
    -1006: RequiredFieldMissing as required_field_missing(field)
            => "Field is required but missing: '{field}'.",
    -1007: NonWebAuthnRequestDenied as non_webauthn_request_denied(endpoint)
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

pub use define_application_many_error;

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
pub struct Reason<T> {
    code: T,
    message: Option<String>,
    arguments: BTreeMap<String, String>,
}

impl<T> Reason<T> {
    #[inline]
    pub const fn new(
        code: T,
        message: Option<String>,
        arguments: BTreeMap<String, String>,
    ) -> Self {
        Self {
            code,
            message,
            arguments,
        }
    }

    #[inline]
    pub const fn code(&self) -> &T {
        &self.code
    }

    #[inline]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    #[inline]
    pub fn argument<S: AsRef<str>>(&self, field: S) -> Option<&str> {
        self.arguments.get(field.as_ref()).map(|x| x.as_str())
    }

    #[inline]
    pub fn arguments(&self) -> &BTreeMap<String, String> {
        &self.arguments
    }
}

impl<T: Encode> Reason<T> {
    #[inline]
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        minicbor::encode(self, &mut bytes).map_err(|e| format!("{}", e))?;
        Ok(bytes)
    }
}

impl<'b, T: Decode<'b> + Default> Reason<T> {
    #[inline]
    pub fn from_bytes(bytes: &'b [u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }
}

impl<T: Display> Display for Reason<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = self
            .message
            .clone()
            .unwrap_or_else(|| format!("Error '{}'", self.code));

        let re = regex::Regex::new(r"\{\{|\}\}|\{[^\}\s]*\}").unwrap();
        let mut current = 0;

        for mat in re.find_iter(&message) {
            let std::ops::Range { start, end } = mat.range();
            f.write_str(&message[current..start])?;
            current = end;

            let s = mat.as_str();
            if s == "{{" {
                f.write_str("{")?;
            } else if s == "}}" {
                f.write_str("}")?;
            } else {
                let field = &message[start + 1..end - 1];
                f.write_str(
                    self.arguments
                        .get(field)
                        .unwrap_or(&"".to_string())
                        .as_str(),
                )?;
            }
        }
        f.write_str(&message[current..])
    }
}

impl<T: Encode> Encode for Reason<T> {
    #[inline]
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.map(
            1 + if self.message.is_none() { 0 } else { 1 }
                + if self.arguments.is_empty() { 0 } else { 1 },
        )?
        .u32(ReasonCborKey::Code as u32)?
        .encode(&self.code)?;

        if let Some(msg) = &self.message {
            e.u32(ReasonCborKey::Message as u32)?.str(msg.as_str())?;
        }
        if !self.arguments.is_empty() {
            e.u32(ReasonCborKey::Arguments as u32)?
                .encode(&self.arguments)?;
        }
        Ok(())
    }
}

impl<'b, T: Decode<'b> + Default> Decode<'b> for Reason<T> {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let len = d.map()?;

        let mut code: Option<T> = None;
        let mut message = None;
        let mut arguments: BTreeMap<String, String> = BTreeMap::new();

        let mut i = 0;
        loop {
            if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            match num_traits::FromPrimitive::from_i64(d.i64()?) {
                Some(ReasonCborKey::Code) => code = Some(d.decode()?),
                Some(ReasonCborKey::Message) => message = Some(d.str()?),
                Some(ReasonCborKey::Arguments) => arguments = d.decode()?,
                None => {}
            }

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        Ok(Self {
            code: code.unwrap_or_default(),
            message: message.map(|s| s.to_string()),
            arguments,
        })
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
    pub fn argument<S: AsRef<str>>(&self, field: S) -> Option<&str> {
        self.0.argument(field)
    }

    #[inline]
    pub fn arguments(&self) -> &BTreeMap<String, String> {
        self.0.arguments()
    }

    #[inline]
    pub const fn is_attribute_specific(&self) -> bool {
        self.0.code.is_attribute_specific()
    }

    #[inline]
    pub const fn is_application_specific(&self) -> bool {
        self.0.code.is_application_specific()
    }

    pub const fn new(
        code: ManyErrorCode,
        message: Option<String>,
        arguments: BTreeMap<String, String>,
    ) -> Self {
        Self(Reason::new(code, message, arguments))
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

impl Encode for ManyError {
    #[inline]
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.encode(&self.0)?;
        Ok(())
    }
}
impl<'b> Decode<'b> for ManyError {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        Ok(Self(d.decode()?))
    }
}

#[cfg(test)]
mod tests {
    use super::ManyError;
    use crate::message::error::ManyErrorCode as ErrorCode;
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

        assert_eq!(format!("{}", e), "Hello ZERO and TWO.");
    }

    #[test]
    fn works_with_only_replacement() {
        let mut arguments = BTreeMap::new();
        arguments.insert("0".to_string(), "ZERO".to_string());
        arguments.insert("1".to_string(), "ONE".to_string());
        arguments.insert("2".to_string(), "TWO".to_string());

        let e = ManyError::new(ErrorCode::Unknown, Some("{2}".to_string()), arguments);

        assert_eq!(format!("{}", e), "TWO");
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

        assert_eq!(format!("{}", e), "@.");
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

        assert_eq!(format!("{}", e), "/{}{ZERO}{}}{TWO.");
    }
}
