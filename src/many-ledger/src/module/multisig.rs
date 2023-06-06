use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::account::features::multisig;
use many_modules::EmptyReturn;
use many_protocol::ResponseMessage;
use minicbor::bytes::ByteVec;

impl multisig::AccountMultisigModuleBackend for LedgerModuleImpl {
    fn multisig_submit_transaction(
        &mut self,
        sender: &Address,
        arg: multisig::SubmitTransactionArgs,
    ) -> Result<multisig::SubmitTransactionReturn, ManyError> {
        let token = self.storage.create_multisig_transaction(sender, arg)?;
        Ok(multisig::SubmitTransactionReturn {
            token: ByteVec::from(token),
        })
    }

    fn multisig_info(
        &self,
        _sender: &Address,
        args: multisig::InfoArgs,
    ) -> Result<multisig::InfoReturn, ManyError> {
        let info = self.storage.get_multisig_info(&args.token)?;
        Ok(info.info)
    }

    fn multisig_set_defaults(
        &mut self,
        sender: &Address,
        args: multisig::SetDefaultsArgs,
    ) -> Result<multisig::SetDefaultsReturn, ManyError> {
        self.storage
            .set_multisig_defaults(sender, args)
            .map(|_| EmptyReturn)
    }

    fn multisig_approve(
        &mut self,
        sender: &Address,
        args: multisig::ApproveArgs,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .approve_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }

    fn multisig_revoke(
        &mut self,
        sender: &Address,
        args: multisig::RevokeArgs,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .revoke_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }

    fn multisig_execute(
        &mut self,
        sender: &Address,
        args: multisig::ExecuteArgs,
    ) -> Result<ResponseMessage, ManyError> {
        self.storage.execute_multisig(sender, args.token.as_slice())
    }

    fn multisig_withdraw(
        &mut self,
        sender: &Address,
        args: multisig::WithdrawArgs,
    ) -> Result<EmptyReturn, ManyError> {
        self.storage
            .withdraw_multisig(sender, args.token.as_slice())
            .map(|_| EmptyReturn)
    }
}
