use coset::CoseSign1;
use many_error::ManyError;
use many_identity::Address;
use many_modules::{ledger, ManyModule, ManyModuleInfo};
use many_protocol::{RequestMessage, ResponseMessage};
use std::collections::BTreeSet;
use std::fmt::{Debug, Formatter};

pub struct AllowAddrsModule<T: ledger::LedgerCommandsModuleBackend> {
    pub inner: ledger::LedgerCommandsModule<T>,
    pub allow_addrs: BTreeSet<Address>,
}

impl<T: ledger::LedgerCommandsModuleBackend> Debug for AllowAddrsModule<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("AllowAddrsModule")
    }
}

#[async_trait::async_trait]
impl<T: ledger::LedgerCommandsModuleBackend> ManyModule for AllowAddrsModule<T> {
    fn info(&self) -> &ManyModuleInfo {
        self.inner.info()
    }

    fn validate(&self, message: &RequestMessage, envelope: &CoseSign1) -> Result<(), ManyError> {
        self.inner.validate(message, envelope)
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError> {
        if !self.allow_addrs.contains(&message.from()) {
            return Err(ManyError::invalid_from_identity());
        }

        self.inner.execute(message).await
    }
}
