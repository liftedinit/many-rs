use crate::error;
use crate::module::account::verify_account_role;
use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::account::features::TryCreateFeature;
use many_modules::account::Role;
use many_modules::{account, ledger, EmptyReturn};

impl ledger::LedgerCommandsModuleBackend for LedgerModuleImpl {
    fn send(&mut self, sender: &Address, args: ledger::SendArgs) -> Result<EmptyReturn, ManyError> {
        let ledger::SendArgs {
            from,
            to,
            amount,
            symbol,
            memo,
        } = args;

        let from = from.as_ref().unwrap_or(sender);
        // We check here to make sure there isn't a code path that might ends up here without
        // proper validation (e.g. multisig or delayed execution). This should normally
        // not be a problem unless you have an instance of the module directly.
        if from.is_illegal() {
            return Err(error::unauthorized());
        }
        if from != sender {
            if let Some(account) = self.storage.get_account(from)? {
                verify_account_role(
                    &account,
                    sender,
                    account::features::ledger::AccountLedger::ID,
                    [Role::CanLedgerTransact],
                )?;
            } else {
                return Err(error::unauthorized());
            }
        }

        self.storage.send(from, &to, &symbol, amount, memo)?;
        Ok(EmptyReturn)
    }
}
