use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_modules::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, BeginBlockReturn, EndpointInfo, InitChainReturn,
    ManyAbciModuleBackend,
};
use many_types::Timestamp;
use std::collections::BTreeMap;
use tracing::info;

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl ManyAbciModuleBackend for LedgerModuleImpl {
    #[rustfmt::skip]
    fn init(&mut self) -> Result<AbciInit, ManyError> {
        Ok(AbciInit {
            endpoints: BTreeMap::from([
                ("ledger.info".to_string(), EndpointInfo { is_command: false }),
                ("ledger.balance".to_string(), EndpointInfo { is_command: false }),
                ("ledger.send".to_string(), EndpointInfo { is_command: true }),

                // Events
                ("events.info".to_string(), EndpointInfo { is_command: false }),
                ("events.list".to_string(), EndpointInfo { is_command: false }),

                // IdStore
                ("idstore.store".to_string(), EndpointInfo { is_command: true }),
                ("idstore.getFromRecallPhrase".to_string(), EndpointInfo { is_command: false }),
                ("idstore.getFromAddress".to_string(), EndpointInfo { is_command: false }),

                // Accounts
                ("account.create".to_string(), EndpointInfo { is_command: true }),
                ("account.setDescription".to_string(), EndpointInfo { is_command: true }),
                ("account.listRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.getRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.addRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.removeRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.info".to_string(), EndpointInfo { is_command: false }),
                ("account.disable".to_string(), EndpointInfo { is_command: true }),
                ("account.addFeatures".to_string(), EndpointInfo { is_command: true }),

                // Account Features - Multisig
                ("account.multisigSetDefaults".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigSubmitTransaction".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigInfo".to_string(), EndpointInfo { is_command: false }),
                ("account.multisigApprove".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigRevoke".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigExecute".to_string(), EndpointInfo { is_command: true }),
                ("account.multisigWithdraw".to_string(), EndpointInfo { is_command: true }),

                // Data Attributes
                ("data.info".to_string(), EndpointInfo { is_command: false }),
                ("data.getInfo".to_string(), EndpointInfo { is_command: false }),
                ("data.query".to_string(), EndpointInfo { is_command: false }),

                // Token attribute
                ("tokens.create".to_string(), EndpointInfo { is_command : true }),
                ("tokens.update".to_string(), EndpointInfo { is_command : true }),
                ("tokens.info".to_string(), EndpointInfo { is_command : false }),
                ("tokens.addExtendedInfo".to_string(), EndpointInfo { is_command : true }),
                ("tokens.removeExtendedInfo".to_string(), EndpointInfo { is_command : true }),
                ("tokens.mint".to_string(), EndpointInfo { is_command : true }),
                ("tokens.burn".to_string(), EndpointInfo { is_command : true }),
            ]),
        })
    }

    fn init_chain(&mut self) -> Result<InitChainReturn, ManyError> {
        info!("abci.init_chain()",);
        Ok(InitChainReturn {})
    }

    fn begin_block(&mut self, info: AbciBlock) -> Result<BeginBlockReturn, ManyError> {
        let time = info.time;
        info!(
            "abci.block_begin(): time={:?} curr_height={}",
            time,
            self.storage.get_height()?
        );

        if let Some(time) = time {
            let time = Timestamp::new(time)?;
            self.storage.set_time(time);
        }

        Ok(BeginBlockReturn {})
    }

    fn info(&self) -> Result<AbciInfo, ManyError> {
        let storage = &self.storage;
        let height = storage.get_height()?;

        info!(
            "abci.info(): height={} hash={}",
            height,
            hex::encode(storage.hash()).as_str()
        );
        Ok(AbciInfo {
            height,
            hash: storage.hash().into(),
        })
    }

    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError> {
        let result = self.storage.commit();

        info!(
            "abci.commit(): retain_height={} hash={}",
            result.retain_height,
            hex::encode(result.hash.as_slice()).as_str()
        );
        Ok(result)
    }
}
