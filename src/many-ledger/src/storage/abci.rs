use crate::storage::event::HEIGHT_EVENTID_SHIFT;
use crate::storage::LedgerStorage;
use many_modules::abci_backend::AbciCommitInfo;
use many_modules::events::EventId;

impl LedgerStorage {
    pub fn commit(&mut self) -> AbciCommitInfo {
        // First check if there's any need to clean up multisig transactions. Ignore
        // errors.
        let _ = self.check_timed_out_multisig_transactions();

        let height = self.inc_height().expect("Unable to increment height.");
        let retain_height = 0;

        // Committing before the migration so that the migration has
        // the actual state of the database when setting its
        // attributes.
        self.commit_storage().expect("Unable to commit to storage.");

        // Initialize/update migrations at current height, if any
        self.migrations
            .update_at_height(&mut self.persistent_store, height + 1)
            .expect("Unable to run migrations");

        self.commit_storage().expect("Unable to commit to storage.");

        let hash = self.persistent_store.root_hash().to_vec();
        self.current_hash = Some(hash.clone());

        self.latest_tid = EventId::from(height << HEIGHT_EVENTID_SHIFT);

        AbciCommitInfo {
            retain_height,
            hash: hash.into(),
        }
    }
}
