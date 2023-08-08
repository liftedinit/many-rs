use cucumber::{given, then, when, World as _};
use many_identity::testing::identity;
use many_modules::kvstore::{GetArgs, KvStoreModuleBackend};
use many_modules::web::{DeployArgs, ListArgs, WebCommandsModuleBackend, WebModuleBackend};
use many_types::web::{WebDeploymentFilter, WebDeploymentSource};
use many_web::module::{InitialStateJson, WebModuleImpl};
use many_web::storage::HTTP_ROOT;
use std::path::Path;
use cucumber::gherkin::Step;
use tempfile::Builder;

#[derive(cucumber::World, Debug)]
#[world(init = Self::new)]
struct World {
    site_name: String,
    site_description: Option<String>,
    source: WebDeploymentSource,
    module: WebModuleImpl,
}

impl World {
    fn new() -> Self {
        Self {
            site_name: "".to_string(),
            site_description: None,
            source: WebDeploymentSource::Zip(vec![].into()),
            module: WebModuleImpl::new(
                InitialStateJson::default(),
                Builder::new()
                    .prefix("many-web")
                    .tempdir()
                    .expect("Unable to create temporary directory"),
                false,
            )
            .expect("Unable to create web module"),
        }
    }
}

#[given(expr = "a website name {string}")]
fn given_site_name(w: &mut World, name: String) {
    w.site_name = name;
}

#[given(expr = "a website description {string}")]
fn given_site_description(w: &mut World, description: String) {
    w.site_description = Some(description);
}

#[given(expr = "a website zip source {string}")]
fn given_site_source(w: &mut World, source: String) {
    let b = hex::decode(source).expect("Unable to decode hex string");
    match &mut w.source {
        WebDeploymentSource::Zip(bytes) => *bytes = b.into(),
    }
}

#[when(expr = "the website is deployed as identity {int}")]
fn when_deploy(w: &mut World, seed: u32) {
    w.module
        .deploy(
            &identity(seed),
            DeployArgs {
                site_name: w.site_name.clone(),
                site_description: w.site_description.clone(),
                source: w.source.clone(),
            },
        )
        .expect("Website deployment failed");
}

#[when(expr = "the website {string} is removed as identity {int}")]
fn when_remove(w: &mut World, site_name: String, seed: u32) {
    w.module
        .remove(
            &identity(seed),
            many_modules::web::RemoveArgs { site_name },
        )
        .expect("Website removal failed");
}

#[then(expr = "the {string} value of website {string} for owner identity {int} is")]
fn then_live(w: &mut World, step: &Step, file: String, site_name: String, seed: u32) {
    let value = step.docstring().expect("Docstring is empty");
    let ret = w
        .module
        .get(
            &identity(0),
            GetArgs {
                key: format!(
                    "{}/{}/{}/{}",
                    HTTP_ROOT,
                    identity(seed),
                    site_name,
                    file
                )
                .into_bytes()
                .into(),
            },
        )
        .expect("Website not found");
    assert_eq!(ret.value.expect("Key is empty"), value.clone().into_bytes().into());
}

#[then(expr = "{string} of website {string} for identity {int} is empty")]
fn then_empty(w: &mut World, file: String, site_name: String, seed: u32) {
    let ret = w
        .module
        .get(
            &identity(seed),
            GetArgs {
                key: format!(
                    "{}/{}/{}/{}",
                    HTTP_ROOT,
                    identity(seed),
                    site_name,
                file).into_bytes().into()
            },
        )
        .expect("Website not found");
    assert_eq!(ret.value, None);
}

#[then(expr = "the website list should contain {string}")]
fn then_list(w: &mut World, site_name: String) {
    let ret =
    WebModuleBackend::list(&w.module, &identity(0), ListArgs { order: None, filter: None })
        .expect("Website list failed");
    assert!(ret.deployments.into_iter().any(|v| v.site_name == site_name));
}

#[then(expr = "the website list filtered by identity {int} should contain {string}")]
fn then_list_filtered(w: &mut World, seed: u32, site_name: String) {
    let ret =
        WebModuleBackend::list(&w.module, &identity(0), ListArgs { order: None, filter: Some(vec![WebDeploymentFilter::Owner(identity(seed))]) })
            .expect("Website list failed");
    assert!(ret.deployments.into_iter().any(|v| v.site_name == site_name));
}

#[then(expr = "the website list should not contain {string}")]
fn then_list_not(w: &mut World, site_name: String) {
    let ret =
        WebModuleBackend::list(&w.module, &identity(0), ListArgs { order: None, filter: None })
            .expect("Website list failed");
    assert!(ret.deployments.into_iter().any(|v| v.site_name != site_name));
}

#[then(expr = "the website list filtered by identity {int} should not contain {string}")]
fn then_list_not_filtered(w: &mut World, seed: u32, site_name: String) {
    let ret =
        WebModuleBackend::list(&w.module, &identity(0), ListArgs { order: None, filter: Some(vec![WebDeploymentFilter::Owner(identity(seed))]) })
            .expect("Website list failed");
    assert!(ret.deployments.into_iter().any(|v| v.site_name != site_name));
}

#[then(expr = "the website deployment fails with {string}")]
fn then_deployment_failed(w: &mut World, error: String) {
    assert!(matches!(
        w.module.deploy(
            &identity(0),
            DeployArgs {
                site_name: w.site_name.clone(),
                site_description: w.site_description.clone(),
                source: w.source.clone(),
            },
        ),
        Err(e) if e.to_string() == error
    ));
}

#[tokio::main]
async fn main() {
    // Support both Cargo and Bazel paths
    let features = ["tests/features", "src/many-web/tests/features"]
        .into_iter()
        .find(|&p| Path::new(p).exists())
        .expect("Cucumber test features not found");

    World::run(Path::new(features).join("web.feature")).await;
}
