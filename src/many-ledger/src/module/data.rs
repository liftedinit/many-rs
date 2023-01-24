use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::data::{
    DataGetInfoArgs, DataGetInfoReturns, DataInfoArgs, DataInfoReturns, DataModuleBackend,
    DataQueryArgs, DataQueryReturns,
};

impl DataModuleBackend for LedgerModuleImpl {
    fn info(&self, _: &Address, _: DataInfoArgs) -> Result<DataInfoReturns, ManyError> {
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
    ) -> Result<DataGetInfoReturns, ManyError> {
        let filtered = self
            .storage
            .data_info()?
            .unwrap_or_default()
            .into_iter()
            .filter(|(k, _)| args.indices.0.contains(k))
            .collect();
        Ok(filtered)
    }

    fn query(&self, _sender: &Address, args: DataQueryArgs) -> Result<DataQueryReturns, ManyError> {
        let filtered = self
            .storage
            .data_attributes()?
            .unwrap_or_default()
            .into_iter()
            .filter(|(k, _)| args.indices.0.contains(k))
            .collect();
        Ok(filtered)
    }
}
