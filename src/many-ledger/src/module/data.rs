use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::data::{
    DataGetInfoArgs, DataGetInfoReturns, DataInfoArgs, DataInfoReturns, DataModuleBackend,
    DataQueryArgs, DataQueryReturns,
};
use many_protocol::context::Context;

impl DataModuleBackend for LedgerModuleImpl {
    fn info(
        &self,
        _: &Address,
        _: DataInfoArgs,
        context: Context,
    ) -> Result<DataInfoReturns, ManyError> {
        self.storage.prove_state(
            context,
            vec![crate::storage::data::DATA_ATTRIBUTES_KEY.into()],
        )?;
        Ok(DataInfoReturns {
            indices: self
                .storage
                .data_attributes()?
                .unwrap_or_default()
                .into_keys()
                .collect(),
        })
    }

    fn get_info(
        &self,
        _sender: &Address,
        args: DataGetInfoArgs,
        context: Context,
    ) -> Result<DataGetInfoReturns, ManyError> {
        let filtered = self
            .storage
            .data_info()?
            .unwrap_or_default()
            .into_iter()
            .filter(|(k, _)| args.indices.0.contains(k))
            .collect();
        self.storage
            .prove_state(context, vec![crate::storage::data::DATA_INFO_KEY.into()])
            .map(|_| filtered)
    }

    fn query(
        &self,
        _sender: &Address,
        args: DataQueryArgs,
        context: Context,
    ) -> Result<DataQueryReturns, ManyError> {
        let filtered = self
            .storage
            .data_attributes()?
            .unwrap_or_default()
            .into_iter()
            .filter(|(k, _)| args.indices.0.contains(k))
            .collect();
        self.storage
            .prove_state(
                context,
                vec![crate::storage::data::DATA_ATTRIBUTES_KEY.into()],
            )
            .map(|_| filtered)
    }
}
