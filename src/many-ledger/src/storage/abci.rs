use {
    crate::storage::event::HEIGHT_EVENTID_SHIFT,
    crate::storage::{InnerStorage, LedgerStorage},
    many_error::ManyError,
    many_modules::abci_backend::AbciCommitInfo,
    many_modules::events::EventId,
    minicbor::bytes::ByteVec,
    std::path::PathBuf,
};

impl LedgerStorage {
    pub fn commit(&mut self) -> AbciCommitInfo {
        let (retain_height, hash) = (|| -> Result<(u64, ByteVec), ManyError> {
            // First check if there's any need to clean up multisig transactions. Ignore
            // errors.
            let _ = self.check_timed_out_multisig_transactions();

            let height = self.inc_height().unwrap();
            let retain_height = 0;

            // Committing before the migration so that the migration has
            // the actual state of the database when setting its
            // attributes.
            self.commit_storage().unwrap();

            // Initialize/update migrations at current height, if any
            self.migrations
                .update_at_height(
                    &mut self.persistent_store,
                    || {
                        InnerStorage::open_v2(["/tmp", "v2_storage"].iter().collect::<PathBuf>())
                            .map_err(ManyError::unknown)
                    },
                    height + 1,
                    self.path.clone(),
                )
                .unwrap();

            self.commit_storage().unwrap();

            let hash = self.persistent_store.root_hash().to_vec();
            self.current_hash = Some(hash.clone());

            self.latest_tid = EventId::from(height << HEIGHT_EVENTID_SHIFT);
            Ok((retain_height, hash.into()))
        })()
        .unwrap();

        // TODO: This function's implementation proves that the return type of
        // LedgerModuleImpl's trait method should be Result<AbciCommitInfo, ManyError>
        AbciCommitInfo {
            retain_height,
            hash,
        }
    }
}
