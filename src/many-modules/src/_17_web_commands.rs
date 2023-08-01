use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;

pub mod deploy;
pub mod remove;

pub use deploy::*;
pub use remove::*;

#[cfg(test)]
use mockall::{automock, predicate::*};

#[many_module(name = WebCommandsModule, id = 17, namespace = web, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait WebCommandsModuleBackend: Send {
    #[many(deny_anonymous)]
    fn deploy(&mut self, sender: &Address, args: DeployArgs) -> Result<DeployReturns, ManyError>;

    #[many(deny_anonymous)]
    fn remove(&mut self, sender: &Address, args: RemoveArgs) -> Result<RemoveReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use crate::testutils::call_module_cbor;
    use std::sync::{Arc, Mutex};
    use mockall::predicate;
    use many_identity::testing::identity;
    use many_types::web::WebDeploymentSource;
    use crate::web::{DeployArgs, DeployReturns, MockWebCommandsModuleBackend, RemoveArgs, RemoveReturns};

    #[test]
    fn deploy() {
        let mut mock = MockWebCommandsModuleBackend::new();
        let data = DeployArgs {
            site_name: "".to_string(),
            site_description: None,
            source: WebDeploymentSource::GitHub("".to_string(), None),
        };
        mock.expect_deploy()
            .with(predicate::eq(identity(1)), predicate::eq(data.clone()))
            .times(1).returning(|_sender, _args| {
            Ok(DeployReturns { url: "foobar".to_string() })
        });
        let module = super::WebCommandsModule::new(Arc::new(Mutex::new(mock)));

        let deploy: DeployReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "web.deploy",
                minicbor::to_vec(data).unwrap(),
            )
                .unwrap(),
        )
            .unwrap();
        assert_eq!(deploy.url, "foobar".to_string());
    }

    #[test]
    fn remove() {
        let mut mock = MockWebCommandsModuleBackend::new();
        let data = RemoveArgs {
            site_name: "foobar".to_string(),
        };
        mock.expect_remove()
            .with(predicate::eq(identity(1)), predicate::eq(data.clone()))
            .times(1).returning(|_sender, _args| {
            Ok(RemoveReturns {})
        });
        let module = super::WebCommandsModule::new(Arc::new(Mutex::new(mock)));

        let _: RemoveReturns = minicbor::decode(
            &call_module_cbor(
                1,
                &module,
                "web.remove",
                minicbor::to_vec(data)
                    .unwrap(),
            )
                .unwrap(),
        )
            .unwrap();
    }
}
