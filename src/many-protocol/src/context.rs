use {
    crate::RequestMessage,
    async_channel::{Sender, TrySendError},
    many_error::ManyError,
    many_types::{attributes::Attribute, ProofOperation, PROOF},
    std::borrow::Cow,
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
    type Item = Attribute;
    type IntoIter = std::vec::IntoIter<Attribute>;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Error(_) | Self::ProofNotRequested => vec![].into_iter(),
            Self::Proof(_) => vec![PROOF].into_iter(),
        }
    }
}

impl Context {
    pub fn prove<P: IntoIterator<Item = ProofOperation>, Prover: FnOnce() -> Result<P, ManyError>>(
        &self,
        prover: Prover,
    ) -> Option<TrySendError<ProofResult>> {
        use ProofResult::{Error, Proof, ProofNotRequested};
        let result = if self.proof_requested() {
            prover()
                .map(Iterator::collect)
                .map(Proof)
                .unwrap_or_else(Error)
        } else {
            ProofNotRequested
        };
        self.transmitter
            .try_send(result)
            .map(|_| None)
            .unwrap_or_else(Some)
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

impl From<Context> for Cow<'_, Context> {
    fn from(context: Context) -> Self {
        Self::Owned(context)
    }
}

impl<'a> From<&'a Context> for Cow<'a, Context> {
    fn from(context: &'a Context) -> Self {
        Self::Borrowed(context)
    }
}
