use coset::CoseSign1;
use many_error::ManyError;
use many_identity::Address;
use many_protocol::RequestMessage;
use many_server::RequestValidator;
use minicbor::Decode;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct WhitelistValidator {
    whitelist: BTreeSet<Address>,
}

impl WhitelistValidator {
    pub fn new(whitelist: PathBuf) -> Self {
        let whitelist = std::fs::read_to_string(whitelist)
            .expect("failed to read whitelist")
            .lines()
            .map(|s| s.parse().expect("failed to parse address"))
            .collect();
        Self { whitelist }
    }
}

impl RequestValidator for WhitelistValidator {
    fn validate_envelope(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        let payload = envelope
            .payload
            .as_ref()
            .ok_or_else(ManyError::empty_envelope)?;
        let request: RequestMessage =
            minicbor::decode(payload).map_err(ManyError::deserialization_error)?;

        if self
            .whitelist
            .contains(&request.from.ok_or(ManyError::invalid_from_identity())?)
        {
            Ok(())
        } else {
            Err(ManyError::duplicated_message())
        }
    }
}
