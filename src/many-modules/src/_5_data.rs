pub mod get_info;
pub mod info;
pub mod query;
pub mod types;
pub use get_info::*;
pub use info::*;
use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;
pub use query::*;
pub use types::*;

#[cfg(test)]
use mockall::{automock, predicate::*};

#[many_module(name = DataModule, id = 5, namespace = data, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait DataModuleBackend: Send {
    fn info(&self, sender: &Address, args: DataInfoArgs) -> Result<DataInfoReturns, ManyError>;
    fn get_info(
        &self,
        sender: &Address,
        args: DataGetInfoArgs,
    ) -> Result<DataGetInfoReturns, ManyError>;
    fn query(&self, sender: &Address, args: DataQueryArgs) -> Result<DataQueryReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;
    use std::sync::{Arc, Mutex};

    use many_types::VecOrSingle;

    use crate::testutils::{call_module, call_module_cbor};

    use super::*;

    fn account_total_count() -> DataIndex {
        accounts_count::TOTAL_COUNT_INDEX
    }

    fn non_zero_account_total_count() -> DataIndex {
        accounts_count::NON_ZERO_TOTAL_COUNT_INDEX
    }

    #[test]
    fn info() {
        let account_total_count = account_total_count();
        let non_zero_account_total_count = non_zero_account_total_count();
        let info_returns = DataInfoReturns {
            indices: vec![account_total_count, non_zero_account_total_count],
        };

        let mut mock = MockDataModuleBackend::new();
        mock.expect_info()
            .times(1)
            .return_const(Ok(info_returns.clone()));
        let module = super::DataModule::new(Arc::new(Mutex::new(mock)));
        let results: DataInfoReturns =
            minicbor::decode(&call_module(5, &module, "data.info", "null").unwrap()).unwrap();

        assert_eq!(info_returns.indices, results.indices);
        assert_eq!(info_returns.indices[0], account_total_count);
        assert_eq!(info_returns.indices[1], non_zero_account_total_count);
    }

    #[test]
    fn get_info() {
        // Arguments
        let account_total_count = account_total_count();
        let non_zero_account_total_count = non_zero_account_total_count();
        let args = DataGetInfoArgs {
            indices: VecOrSingle(vec![account_total_count, non_zero_account_total_count]),
        };

        // Returns
        let atc = DataInfo {
            r#type: DataType::Counter,
            shortname: "accountTotalCount".into(),
        };
        let nzatc = DataInfo {
            r#type: DataType::Counter,
            shortname: "nonZeroAccountTotalCount".into(),
        };
        let mut returns = DataGetInfoReturns::new();
        returns.insert(account_total_count, atc.clone());
        returns.insert(non_zero_account_total_count, nzatc.clone());

        let mut mock = MockDataModuleBackend::new();
        mock.expect_get_info()
            .times(1)
            .return_const(Ok(returns.clone()));
        let module = super::DataModule::new(Arc::new(Mutex::new(mock)));
        let results: DataGetInfoReturns = minicbor::decode(
            &call_module_cbor(5, &module, "data.getInfo", minicbor::to_vec(args).unwrap()).unwrap(),
        )
        .unwrap();

        assert_eq!(results, returns);
        assert_eq!(results[&account_total_count], atc);
        assert_eq!(results[&non_zero_account_total_count], nzatc);
    }

    #[test]
    fn query() {
        // Arguments
        let account_total_count = account_total_count();
        let non_zero_account_total_count = non_zero_account_total_count();
        let args = DataQueryArgs {
            indices: VecOrSingle(vec![account_total_count, non_zero_account_total_count]),
        };

        // Returns
        let mut returns = DataQueryReturns::new();
        let act_value = DataValue::GaugeInt(BigInt::from(10));
        let nzatc_value = DataValue::GaugeInt(BigInt::from(1));
        returns.insert(account_total_count, act_value.clone());
        returns.insert(non_zero_account_total_count, nzatc_value.clone());

        let mut mock = MockDataModuleBackend::new();
        mock.expect_query().times(1).return_const(Ok(returns));
        let module = super::DataModule::new(Arc::new(Mutex::new(mock)));
        let results: DataQueryReturns = minicbor::decode(
            &call_module_cbor(5, &module, "data.query", minicbor::to_vec(args).unwrap()).unwrap(),
        )
        .unwrap();

        let mut ds = DataSet::default().with_known_types().unwrap();
        ds.merge(results.clone()).unwrap();

        // Check that both the dataset and the results match their expected value.
        assert_eq!(&ds.get_value(account_total_count).unwrap(), &act_value);
        assert_eq!(
            &results.get(&account_total_count).unwrap().clone(),
            &act_value
        );
        assert_eq!(
            &ds.get_value(non_zero_account_total_count).unwrap(),
            &nzatc_value
        );
        assert_eq!(
            &results.get(&non_zero_account_total_count).unwrap().clone(),
            &nzatc_value
        );
    }
}
