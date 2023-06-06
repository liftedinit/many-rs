use coset::CoseSign1;
use many_error::ManyError;
use many_modules::{idstore, ManyModule, ManyModuleInfo};
use many_protocol::{RequestMessage, ResponseMessage};
use std::fmt::{Debug, Formatter};

pub struct IdStoreWebAuthnModule<T: idstore::IdStoreModuleBackend> {
    pub inner: idstore::IdStoreModule<T>,
    pub check_webauthn: bool,
}

impl<T: idstore::IdStoreModuleBackend> Debug for IdStoreWebAuthnModule<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("IdStoreWebAuthnModule")
    }
}

#[async_trait::async_trait]
impl<T: idstore::IdStoreModuleBackend> ManyModule for IdStoreWebAuthnModule<T> {
    fn info(&self) -> &ManyModuleInfo {
        self.inner.info()
    }

    fn validate(&self, message: &RequestMessage, envelope: &CoseSign1) -> Result<(), ManyError> {
        let result: Result<(), ManyError> = self.inner.validate(message, envelope);
        if let Err(e) = result {
            if e.code() == ManyError::non_webauthn_request_denied("").code() && !self.check_webauthn
            {
                return Ok(());
            } else {
                return Err(e);
            }
        };
        Ok(())
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError> {
        self.inner.execute(message).await
    }
}
