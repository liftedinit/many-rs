use async_trait::async_trait;
use coset::CoseSign1;
use many_identity::CoseKeyIdentity;
use many_modules::base;
use many_protocol::{ManyError, ManyUrl, ResponseMessage};
use many_server::transport::LowLevelManyRequestHandler;

use crate::MockEntries;

#[derive(Debug)]
pub struct ManyMockServer {
    mock_entries: MockEntries,
    allowed_origins: Option<Vec<ManyUrl>>,
    identity: CoseKeyIdentity,
}

impl ManyMockServer {
    pub fn new(
        mock_entries: MockEntries,
        allowed_origins: Option<Vec<ManyUrl>>,
        identity: CoseKeyIdentity,
    ) -> Self {
        ManyMockServer {
            mock_entries,
            allowed_origins,
            identity,
        }
    }
}

#[async_trait]
impl LowLevelManyRequestHandler for ManyMockServer {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let request =
            many_protocol::decode_request_from_cose_sign1(envelope, self.allowed_origins.clone());
        let id = self.identity.clone();

        let message = request.map_err(|_| "Error processing the request".to_string())?;
        let response = self
            .mock_entries
            .get(&message.method)
            .ok_or_else(|| "No mock entry for that".to_string())?;
        let response = ResponseMessage {
            data: Ok(response.clone()),
            ..Default::default()
        };
        many_protocol::encode_cose_sign1_from_response(response, &id)
    }
}

impl base::BaseModuleBackend for ManyMockServer {
    fn endpoints(&self) -> Result<base::Endpoints, ManyError> {
        Ok(base::Endpoints(
            self.mock_entries.iter().map(|(k, _)| k.clone()).collect(),
        ))
    }

    fn status(&self) -> Result<base::Status, ManyError> {
        let public_key = self.identity.key.clone();
        let identity = self.identity.identity;
        Ok(base::Status {
            version: 1,
            name: "mock server".into(),
            public_key,
            identity,
            attributes: Default::default(),
            extras: Default::default(),
            server_version: None,
            timeout: None,
        })
    }
}
