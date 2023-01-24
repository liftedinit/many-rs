use {
    crate::RequestMessage,
    async_channel::Sender,
    many_error::ManyError,
    many_types::{attributes::AttributeSet, PROOF},
};

pub struct Context<'a> {
    request: &'a RequestMessage,
    sender: &'a Sender<ProofResult>,
}

pub enum ProofResult {
    Error(ManyError),
    Proof(Vec<u8>),
    ProofNotRequested,
}

impl<'a> Context<'a> {
    pub fn prove(&self, prover: impl FnOnce() -> Result<Vec<u8>, ManyError>) -> ProofResult {
        use ProofResult::{Error, Proof, ProofNotRequested};
        if self.request.attributes.contains(&PROOF) {
            prover().map(Proof).unwrap_or_else(Error)
        } else {
            ProofNotRequested
        }
    }
}
