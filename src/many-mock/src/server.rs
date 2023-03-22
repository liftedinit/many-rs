use async_trait::async_trait;
use coset::CoseSign1;
use many_error::ManyError;
use many_identity::verifiers::AnonymousVerifier;
use many_identity::Identity;
use many_identity_dsa::CoseKeyVerifier;
use many_identity_webauthn::WebAuthnVerifier;
use many_modules::base;
use many_protocol::{ManyUrl, ResponseMessage};
use many_server::transport::LowLevelManyRequestHandler;
use std::fmt::Debug;

use crate::MockEntries;

#[derive(Debug)]
pub struct ManyMockServer<I: Identity> {
    mock_entries: MockEntries,
    identity: I,
    verifier: (AnonymousVerifier, CoseKeyVerifier, WebAuthnVerifier),
}

impl<I: Identity> ManyMockServer<I> {
    pub fn new(
        mock_entries: MockEntries,
        allowed_origins: Option<Vec<ManyUrl>>,
        identity: I,
    ) -> Self {
        let verifier = (
            AnonymousVerifier,
            CoseKeyVerifier,
            WebAuthnVerifier::new(allowed_origins),
        );

        ManyMockServer {
            mock_entries,
            identity,
            verifier,
        }
    }
}

#[async_trait]
impl<I: Identity + Debug + Send + Sync> LowLevelManyRequestHandler for ManyMockServer<I> {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let request = many_protocol::decode_request_from_cose_sign1(&envelope, &self.verifier);
        let id = &self.identity;

        let message = request.map_err(|_| "Error processing the request".to_string())?;
        let response = self
            .mock_entries
            .get(&message.method)
            .ok_or_else(|| "No mock entry for that".to_string())?;
        let response = ResponseMessage {
            from: id.address(),
            data: Ok(response.clone()),
            ..Default::default()
        };
        many_protocol::encode_cose_sign1_from_response(response, id).map_err(|e| e.to_string())
    }
}

impl<I: Identity> base::BaseModuleBackend for ManyMockServer<I> {
    fn endpoints(&self) -> Result<base::Endpoints, ManyError> {
        Ok(base::Endpoints(self.mock_entries.keys().cloned().collect()))
    }

    fn status(&self) -> Result<base::Status, ManyError> {
        let public_key = self.identity.public_key();
        let identity = self.identity.address();
        Ok(base::Status {
            version: 1,
            name: "mock server".to_string(),
            public_key,
            identity,
            attributes: Default::default(),
            extras: Default::default(),
            server_version: None,
            timeout: None,
        })
    }
}
