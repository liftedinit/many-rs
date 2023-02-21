use {
    crate::RequestMessage,
    async_channel::Sender,
    many_error::ManyError,
    many_types::{attributes::Attribute, cbor::CborAny, proof::Proof, ProofOperation, PROOF},
};

#[derive(Clone, Debug)]
pub struct Context {
    request: RequestMessage,
    transmitter: Sender<ProofResult>,
}

pub enum ProofResult {
    Error(ManyError),
    Proof(Vec<ProofOperation>),
    ProofNotRequested,
}

impl IntoIterator for ProofResult {
    type Item = Result<Attribute, ManyError>;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Error(_) | Self::ProofNotRequested => vec![].into_iter(),
            Self::Proof(proof) => {
                vec![CborAny::try_from(Proof::from(proof)).map(|any| PROOF.with_argument(any))]
                    .into_iter()
            }
        }
    }
}

impl Context {
    pub fn new(request: RequestMessage, transmitter: Sender<ProofResult>) -> Self {
        Self {
            request,
            transmitter,
        }
    }

    pub fn prove<
        P: IntoIterator<Item = ProofOperation>,
        Prover: FnOnce() -> Result<P, ManyError>,
    >(
        &self,
        prover: Prover,
    ) -> Result<(), ManyError> {
        use ProofResult::{Error, Proof, ProofNotRequested};
        let result = if self.proof_requested() {
            prover()
                .map(IntoIterator::into_iter)
                .map(Iterator::collect)
                .map(Proof)
                .unwrap_or_else(Error)
        } else {
            ProofNotRequested
        };
        self.transmitter
            .try_send(result)
            .map_err(ManyError::unknown)
    }

    pub fn proof_requested(&self) -> bool {
        self.request.attributes.contains(&PROOF)
    }
}

impl AsRef<Context> for Context {
    fn as_ref(&self) -> &Self {
        self
    }
}
