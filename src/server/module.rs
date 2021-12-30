use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Attribute;
use crate::OmniError;
use async_trait::async_trait;
use std::fmt::Debug;

pub mod base;

#[derive(Clone, Debug)]
pub struct OmniModuleInfo {
    /// Returns the name of this module, for logs and metering.
    pub name: String,
    /// Returns a list of all attributes supported by this module.
    pub attributes: Vec<Attribute>,
}

/// A module ran by an omni server.
#[async_trait]
pub trait OmniModule: Sync + Send + Debug {
    /// Returns information about this module.
    fn info(&self) -> &OmniModuleInfo;

    /// Verify that a message is well formed (ACLs, arguments, etc).
    fn validate(&self, _message: &RequestMessage) -> Result<(), OmniError> {
        Ok(())
    }

    /// Execute a message and returns its response.
    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError>;
}
