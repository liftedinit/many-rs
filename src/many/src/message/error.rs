use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use num_derive::{FromPrimitive, ToPrimitive};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::iter::FromIterator;

#[derive(FromPrimitive, ToPrimitive)]
#[repr(i8)]
enum ManyErrorCborKey {
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

        impl ManyError {
            $($(
                #[doc = $description]
                pub fn $snake_name( $($arg: String,)* ) -> Self {
                    let s = Self {
                        code: ManyErrorCode::$name,
                        message: Some($description.to_string()),
                        arguments: BTreeMap::from_iter(vec![
                            $( (stringify!($arg).to_string(), $arg) ),*
                        ]),
                    };

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
      -11: AttributeNotFound as attribute_not_found(id) 
            => "Expected attribute {id} not found.",
      -12: SnapshotNotFound as snapshot_not_found(id) 
            => "Could not compress snapshot, snapshot not found.",
      -13: SnapshotError as snapshot_creation_error(id) 
            => "Could not create snapshot.",
      -14: SnapshotDirError as snapshot_dir_error(id) 
            => "Error locating snapshot directory.",

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
    -1002: CouldNotVerifySignature as could_not_verify_signature()
            => "Signature does not match the public key.",
    -1003: UnknownDestination as unknown_destination(to, this)
            => "Unknown destination for message.\nThis is \"{this}\", message was for \"{to}\".",
    -1004: EmptyEnvelope as empty_envelope()
            => "An envelope must contain a payload.",
    -1005: TimestampOutOfRange as timestamp_out_of_range()
            => "The message's timestamp is out of the accepted range of the server.",
    -1006: RequiredFieldMissing as required_field_missing(field)
            => "Field is required but missing: '{field}'.",

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
            $vis fn $name ( $($var_name: String),* ) -> $crate::ManyError {
                $crate::ManyError::attribute_specific(
                    ($module_id as i32) * -10000i32 - ($id as i32),
                    String::from($message),
                    std::iter::FromIterator::from_iter(vec![
                        $( (stringify!($var_name).to_string(), $var_name) ),*
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
            $vis fn $name ( $($var_name: String),* ) -> $crate::ManyError {
                $crate::ManyError::application_specific(
                    $id as u32,
                    String::from($message),
                    std::iter::FromIterator::from_iter(vec![
                        $( (stringify!($var_name).to_string(), $var_name) ),*
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
    pub fn is_attribute_specific(&self) -> bool {
        matches!(self, ManyErrorCode::AttributeSpecific(_))
    }
    #[inline]
    pub fn is_application_specific(&self) -> bool {
        matches!(self, ManyErrorCode::ApplicationSpecific(_))
    }

    #[inline]
    pub fn message_of(code: i64) -> Option<&'static str> {
        ManyErrorCode::from(code).message()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ManyError {
    pub code: ManyErrorCode,
    pub message: Option<String>,
    pub arguments: BTreeMap<String, String>,
}

impl ManyError {
    #[inline]
    pub fn is_attribute_specific(&self) -> bool {
        self.code.is_attribute_specific()
    }

    #[inline]
    pub fn is_application_specific(&self) -> bool {
        self.code.is_application_specific()
    }

    #[inline]
    pub fn attribute_specific(
        code: i32,
        message: String,
        arguments: BTreeMap<String, String>,
    ) -> Self {
        ManyError {
            code: ManyErrorCode::AttributeSpecific(code),
            message: Some(message),
            arguments,
        }
    }

    #[inline]
    pub fn application_specific(
        code: u32,
        message: String,
        arguments: BTreeMap<String, String>,
    ) -> Self {
        ManyError {
            code: ManyErrorCode::ApplicationSpecific(code),
            message: Some(message),
            arguments,
        }
    }

    #[inline]
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        minicbor::encode(self, &mut bytes).map_err(|e| format!("{}", e))?;
        Ok(bytes)
    }

    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }
}

impl Default for ManyErrorCode {
    #[inline]
    fn default() -> Self {
        ManyErrorCode::Unknown
    }
}

impl Default for ManyError {
    #[inline]
    fn default() -> Self {
        ManyError::unknown("?".to_string())
    }
}

impl Display for ManyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = self
            .message
            .as_deref()
            .unwrap_or_else(|| self.code.message().unwrap_or("Invalid error code."));

        let re = regex::Regex::new(r"\{\{|\}\}|\{[^\}\s]*\}").unwrap();
        let mut current = 0;

        for mat in re.find_iter(message) {
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

impl std::error::Error for ManyError {}

impl Encode for ManyError {
    #[inline]
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        e.map(
            1 + if self.message.is_none() { 0 } else { 1 }
                + if self.arguments.is_empty() { 0 } else { 1 },
        )?
        .u32(ManyErrorCborKey::Code as u32)?
        .i64(self.code.into())?;

        if let Some(msg) = &self.message {
            e.u32(ManyErrorCborKey::Message as u32)?.str(msg.as_str())?;
        }
        if !self.arguments.is_empty() {
            e.u32(ManyErrorCborKey::Arguments as u32)?
                .encode(&self.arguments)?;
        }
        Ok(())
    }
}

impl<'b> Decode<'b> for ManyError {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let len = d.map()?;

        let mut code = None;
        let mut message = None;
        let mut arguments: BTreeMap<String, String> = BTreeMap::new();

        let mut i = 0;
        loop {
            if d.datatype()? == Type::Break {
                d.skip()?;
                break;
            }

            match num_traits::FromPrimitive::from_i64(d.i64()?) {
                Some(ManyErrorCborKey::Code) => code = Some(d.i64()?),
                Some(ManyErrorCborKey::Message) => message = Some(d.str()?),
                Some(ManyErrorCborKey::Arguments) => arguments = d.decode()?,
                None => {}
            }

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        Ok(Self {
            code: code.unwrap_or(0).into(),
            message: message.map(|s| s.to_string()),
            arguments,
        })
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

        let e = ManyError {
            code: ErrorCode::Unknown,
            message: Some("Hello {0} and {2}.".to_string()),
            arguments,
        };

        assert_eq!(format!("{}", e), "Hello ZERO and TWO.");
    }

    #[test]
    fn works_with_only_replacement() {
        let mut arguments = BTreeMap::new();
        arguments.insert("0".to_string(), "ZERO".to_string());
        arguments.insert("1".to_string(), "ONE".to_string());
        arguments.insert("2".to_string(), "TWO".to_string());

        let e = ManyError {
            code: ErrorCode::Unknown,
            message: Some("{2}".to_string()),
            arguments,
        };

        assert_eq!(format!("{}", e), "TWO");
    }

    #[test]
    fn works_for_others() {
        let mut arguments = BTreeMap::new();
        arguments.insert("0".to_string(), "ZERO".to_string());
        arguments.insert("1".to_string(), "ONE".to_string());
        arguments.insert("2".to_string(), "TWO".to_string());

        let e = ManyError {
            code: ErrorCode::Unknown,
            message: Some("@{a}{b}{c}.".to_string()),
            arguments,
        };

        assert_eq!(format!("{}", e), "@.");
    }

    #[test]
    fn supports_double_brackets() {
        let mut arguments = BTreeMap::new();
        arguments.insert("0".to_string(), "ZERO".to_string());
        arguments.insert("1".to_string(), "ONE".to_string());
        arguments.insert("2".to_string(), "TWO".to_string());

        let e = ManyError {
            code: ErrorCode::Unknown,
            message: Some("/{{}}{{{0}}}{{{a}}}{b}}}{{{2}.".to_string()),
            arguments,
        };

        assert_eq!(format!("{}", e), "/{}{ZERO}{}}{TWO.");
    }
}
