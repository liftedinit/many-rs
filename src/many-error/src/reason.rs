use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

#[cfg(feature = "minicbor")]
pub mod minicbor;

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
    pub fn with_code(self, code: T) -> Self {
        Self { code, ..self }
    }

    #[inline]
    pub const fn code(&self) -> &T {
        &self.code
    }

    #[inline]
    pub fn set_code(&mut self, code: T) {
        self.code = code;
    }

    #[inline]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    #[inline]
    pub fn set_message(&mut self, message: Option<String>) {
        self.message = message;
    }

    #[inline]
    pub fn add_argument(&mut self, key: String, value: String) {
        self.arguments.insert(key, value);
    }

    #[inline]
    pub fn remove_argument(&mut self, key: &String) {
        self.arguments.remove(key);
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
