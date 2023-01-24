use {
    crate::RequestMessage,
    async_channel::Sender,
    many_error::ManyError,
    many_types::{attributes::Attribute, PROOF},
};

pub struct Context<'a> {
    request: &'a RequestMessage,
    transmitter: &'a Sender<ProofResult>,
}

#[derive(Clone)]
pub enum ProofResult {
    Error(ManyError),
    Proof(Vec<u8>),
    ProofNotRequested,
}

impl IntoIterator for ProofResult {
    type Item = Attribute;
    type IntoIter = std::vec::IntoIter<Attribute>;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Error(_) | Self::ProofNotRequested => vec![].into_iter(),
            Self::Proof(_) => vec![PROOF].into_iter(),
        }
    }
}

impl<'a> Context<'a> {
    pub fn prove(&self, prover: impl FnOnce() -> Result<Vec<u8>, ManyError>) -> ProofResult {
        use ProofResult::{Error, Proof, ProofNotRequested};
        let result = if self.request.attributes.contains(&PROOF) {
            prover().map(Proof).unwrap_or_else(Error)
        } else {
            ProofNotRequested
        };
        self.transmitter
            .try_send(result.clone())
            .map(|_| result)
            .unwrap_or_else(|e| Error(ManyError::unknown(e.to_string())))
    }
}

impl<'a> From<(&'a RequestMessage, &'a Sender<ProofResult>)> for Context<'a> {
    fn from((request, transmitter): (&'a RequestMessage, &'a Sender<ProofResult>)) -> Self {
        Self {
            request,
            transmitter,
        }
    }
}
