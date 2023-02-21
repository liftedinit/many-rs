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
            Self::Proof(proof) => vec![minicbor::to_vec(Proof::from(proof))
                .map_err(|error| ManyError::unknown(error.to_string()))
                .and_then(|bytes| {
                    minicbor::decode::<CborAny>(bytes.as_slice())
                        .map_err(|error| ManyError::unknown(error.to_string()))
                })
                .map(|any| PROOF.with_argument(any))]
            .into_iter(),
        }
    }
}

impl Context {
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

impl From<(RequestMessage, Sender<ProofResult>)> for Context {
    fn from((request, transmitter): (RequestMessage, Sender<ProofResult>)) -> Self {
        Self {
            request,
            transmitter,
        }
    }
}

impl AsRef<Context> for Context {
    fn as_ref(&self) -> &Self {
        self
    }
}
