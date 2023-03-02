use many_error::ManyError;
use std::fmt::{Display, Formatter};

/// An error that happened during communication with the server or in the client.
pub enum ClientServerError {
    /// The server returned an error.
    Server(ManyError),

    /// An error happened on the client, either before or after the request/
    /// response were sent/received. Could be deserialization or configuration
    /// or transport error.
    Client(anyhow::Error),
}

impl Display for ClientServerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientServerError::Server(err) => f.write_fmt(format_args!(
                "Error returned by server:\nCode: {} ({:?})\nMessage:\n|  {}\n",
                Into::<i64>::into(err.code()),
                err.code().to_string(),
                err.to_string()
                    .split('\n')
                    .collect::<Vec<&str>>()
                    .join("\n|  ")
            )),
            ClientServerError::Client(err) => {
                f.write_fmt(format_args!("An error happened on the client:\n{err}"))
            }
        }
    }
}

impl From<ManyError> for ClientServerError {
    fn from(value: ManyError) -> Self {
        Self::Server(value)
    }
}

impl From<anyhow::Error> for ClientServerError {
    fn from(value: anyhow::Error) -> Self {
        Self::Client(value)
    }
}

impl From<minicbor::decode::Error> for ClientServerError {
    fn from(value: minicbor::decode::Error) -> Self {
        Self::Client(anyhow::Error::from(value))
    }
}
