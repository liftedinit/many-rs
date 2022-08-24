use many_modules::base::{Endpoints, Status};
use many_protocol::ManyError;

use crate::ManyClient;

#[derive(Clone, Debug)]
pub struct BaseClient {
    client: ManyClient,
}

impl BaseClient {
    pub fn new(client: ManyClient) -> Self {
        BaseClient { client }
    }

    pub async fn status(&self) -> Result<Status, ManyError> {
        let response = self.client.call_("status", ()).await?;
        minicbor::decode(&response).map_err(ManyError::deserialization_error)
    }

    pub async fn heartbeat(&self) -> Result<(), ManyError> {
        self.client.call("heartbeat", ()).await?;
        Ok(())
    }

    pub async fn endpoints(&self) -> Result<Endpoints, ManyError> {
        let response = self.client.call_("endpoints", ()).await?;
        minicbor::decode(&response).map_err(ManyError::deserialization_error)
    }
}
