use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

mod transfer;

pub use transfer::*;

#[many_module(name = KvStoreTransferModule, id = 13, namespace = kvstore, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait KvStoreTransferModuleBackend: Send {
    #[many(deny_anonymous)]
    fn transfer(
        &mut self,
        sender: &Address,
        args: TransferArgs,
    ) -> Result<TransferReturn, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::call_module_cbor;
    use many_identity::testing::identity;
    use minicbor::bytes::ByteVec;
    use std::sync::{Arc, Mutex};

    #[test]
    fn transfer() {
        let data = TransferArgs {
            key: ByteVec::from(vec![1]),
            alternative_owner: None,
            new_owner: Default::default(),
        };

        let mut mock = MockKvStoreTransferModuleBackend::new();
        mock.expect_transfer()
            .with(eq(identity(1)), eq(data.clone()))
            .times(1)
            .returning(|_sender, _args| Ok(TransferReturn {}));
        let module = super::KvStoreTransferModule::new(Arc::new(Mutex::new(mock)));

        let _: TransferReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "kvstore.transfer",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }
}
