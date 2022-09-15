use crate::EmptyReturn;
use crate::ManyError;
use many_macros::many_module;
use minicbor::bytes::ByteVec;
use minicbor::{Decode, Encode};
use std::collections::BTreeMap;

#[cfg(test)]
use mockall::{automock, predicate::*};

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct EndpointInfo {
    #[n(0)]
    pub is_command: bool,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct AbciInit {
    /// List the methods supported by this module. For performance reason, this list will be
    /// cached and the only calls that will be sent to the backend module will be those
    /// listed in this list at initialization.
    /// This list is not private. If the MANY Module needs to have some private endpoints,
    /// it should be implementing those separately. ABCI is not very compatible with private
    /// endpoints as it can't know if they change the state or not.
    #[n(0)]
    pub endpoints: BTreeMap<String, EndpointInfo>,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct AbciInfo {
    #[n(0)]
    pub height: u64,

    #[n(1)]
    pub hash: ByteVec,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct AbciBlock {
    #[n(0)]
    pub time: Option<u64>,
}

#[derive(Clone, Debug, Encode, Decode, Eq, PartialEq)]
#[cbor(map)]
pub struct AbciCommitInfo {
    #[n(0)]
    pub retain_height: u64,

    #[n(1)]
    pub hash: ByteVec,
}

pub type InitChainReturn = EmptyReturn;
pub type BeginBlockReturn = EmptyReturn;
pub type EndBlockReturn = EmptyReturn;

/// A module that adapt a MANY application to an ABCI-MANY bridge.
/// This module takes a backend (another module) which ALSO implements the ModuleBackend
/// trait, and exposes the `abci.info` and `abci.init` endpoints.
/// This module should only be exposed to the tendermint server's network. It is not
/// considered secure (just like an ABCI app would not).
#[many_module(name = AbciModule, id = 1000, namespace = abci, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait ManyAbciModuleBackend: std::fmt::Debug + Send + Sync {
    /// Called when the ABCI frontend is initialized. No action should be taken here, only
    /// information should be returned. If the ABCI frontend is restarted, this method
    /// will be called again.
    fn init(&mut self) -> Result<AbciInit, ManyError>;

    /// Called at Genesis of the Tendermint blockchain.
    fn init_chain(&mut self) -> Result<InitChainReturn, ManyError>;

    /// Called at the start of a block.
    fn begin_block(&mut self, _info: AbciBlock) -> Result<BeginBlockReturn, ManyError> {
        Ok(BeginBlockReturn {})
    }

    /// Called when info is needed from the backend.
    fn info(&self) -> Result<AbciInfo, ManyError>;

    /// Called at the end of a block.
    fn end_block(&mut self) -> Result<EndBlockReturn, ManyError> {
        Ok(EndBlockReturn {})
    }

    /// Called after a block. The app should take this call and serialize its state.
    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::{call_module, call_module_cbor};
    use mockall::predicate;
    use std::sync::{Arc, Mutex};

    #[test]
    fn init() {
        let init = AbciInit {
            endpoints: BTreeMap::from([("Foo".to_string(), EndpointInfo { is_command: false })]),
        };
        let mut mock = MockManyAbciModuleBackend::new();
        mock.expect_init().times(1).return_const(Ok(init.clone()));
        let module = super::AbciModule::new(Arc::new(Mutex::new(mock)));

        let init_return: AbciInit =
            minicbor::decode(&call_module(1, &module, "abci.init", "null").unwrap()).unwrap();

        assert_eq!(init_return, init);
    }

    #[test]
    fn init_chain() {
        let mut mock = MockManyAbciModuleBackend::new();
        mock.expect_init_chain()
            .times(1)
            .returning(|| Ok(InitChainReturn {}));
        let module = super::AbciModule::new(Arc::new(Mutex::new(mock)));

        let _: InitChainReturn =
            minicbor::decode(&call_module(1, &module, "abci.initChain", "null").unwrap()).unwrap();
    }

    #[test]
    fn begin_block() {
        let data = AbciBlock { time: Some(1) };
        let mut mock = MockManyAbciModuleBackend::new();
        mock.expect_begin_block()
            .with(predicate::eq(data.clone()))
            .times(1)
            .returning(|_| Ok(BeginBlockReturn {}));
        let module = super::AbciModule::new(Arc::new(Mutex::new(mock)));

        let _: BeginBlockReturn = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "abci.beginBlock",
                minicbor::to_vec(data).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn info() {
        let info = AbciInfo {
            height: 1,
            hash: vec![13u8; 8].into(),
        };
        let mut mock = MockManyAbciModuleBackend::new();
        mock.expect_info().times(1).return_const(Ok(info.clone()));
        let module = super::AbciModule::new(Arc::new(Mutex::new(mock)));
        let abci_info: AbciInfo =
            minicbor::decode(&call_module(1, &module, "abci.info", "null").unwrap()).unwrap();

        assert_eq!(abci_info, info);
    }

    #[test]
    fn end_block() {
        let mut mock = MockManyAbciModuleBackend::new();
        mock.expect_end_block()
            .times(1)
            .returning(|| Ok(EndBlockReturn {}));
        let module = super::AbciModule::new(Arc::new(Mutex::new(mock)));
        let _: EndBlockReturn =
            minicbor::decode(&call_module(1, &module, "abci.endBlock", "null").unwrap()).unwrap();
    }

    #[test]
    fn commit() {
        let commit_info = AbciCommitInfo {
            retain_height: 1,
            hash: vec![14u8; 8].into(),
        };
        let mut mock = MockManyAbciModuleBackend::new();
        mock.expect_commit()
            .times(1)
            .return_const(Ok(commit_info.clone()));
        let module = super::AbciModule::new(Arc::new(Mutex::new(mock)));
        let abci_commit_info: AbciCommitInfo =
            minicbor::decode(&call_module(1, &module, "abci.commit", "null").unwrap()).unwrap();

        assert_eq!(abci_commit_info, commit_info);
    }
}
