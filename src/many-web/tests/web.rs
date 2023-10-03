use cucumber::gherkin::Step;
use cucumber::{given, then, when, World as _};
use many_identity::testing::identity;
use many_identity::Address;
use many_modules::kvstore::{GetArgs, KvStoreModuleBackend};
use many_modules::web::{
    DeployArgs, ListArgs, UpdateArgs, WebCommandsModuleBackend, WebModuleBackend,
};
use many_types::web::{WebDeploymentFilter, WebDeploymentSource};
use many_types::Memo;
use many_web::module::{InitialStateJson, WebModuleImpl};
use many_web::storage::HTTP_ROOT;
use std::path::Path;
use tempfile::Builder;

#[derive(cucumber::World, Debug)]
#[world(init = Self::new)]
struct World {
    owner: Option<Address>,
    site_name: String,
    site_description: Option<String>,
    source: WebDeploymentSource,
    module: WebModuleImpl,
    memo: Option<Memo>,
    domain: Option<String>,
}

impl World {
    fn new() -> Self {
        Self {
            owner: None,
            site_name: "".to_string(),
            site_description: None,
            source: WebDeploymentSource::Archive(vec![].into()),
            module: WebModuleImpl::new(
                InitialStateJson::default(),
                Builder::new()
                    .prefix("many-web")
                    .tempdir()
                    .expect("Unable to create temporary directory"),
                false,
            )
            .expect("Unable to create web module"),
            memo: None,
            domain: None,
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
        WebDeploymentSource::Archive(bytes) => *bytes = b.into(),
    }
}

#[given(expr = "a website memo {string}")]
fn given_site_memo(w: &mut World, memo: String) {
    w.memo = Some(Memo::try_from(memo).expect("Unable to parse memo"));
}

#[given(expr = "a website domain {string}")]
fn given_site_domain(w: &mut World, domain: String) {
    w.domain = Some(domain);
}

#[given(expr = "a website owner identity {int}")]
fn given_site_owner(w: &mut World, seed: u32) {
    w.owner = Some(identity(seed));
}

#[when(expr = "the website is deployed as identity {int}")]
fn when_deploy(w: &mut World, seed: u32) {
    w.module
        .deploy(
            &identity(seed),
            DeployArgs {
                owner: w.owner,
                site_name: w.site_name.clone(),
                site_description: w.site_description.clone(),
                source: w.source.clone(),
                memo: w.memo.clone(),
                domain: w.domain.clone(),
            },
        )
        .expect("Website deployment failed");
}

#[when(expr = "the website is updated as identity {int}")]
fn when_update(w: &mut World, seed: u32) {
    w.module
        .update(
            &identity(seed),
            UpdateArgs {
                owner: w.owner,
                site_name: w.site_name.clone(),
                site_description: w.site_description.clone(),
                source: w.source.clone(),
                memo: w.memo.clone(),
                domain: w.domain.clone(),
            },
        )
        .expect("Website update failed");
}

#[when(expr = "the website {string} is removed as identity {int}")]
fn when_remove(w: &mut World, site_name: String, seed: u32) {
    w.module
        .remove(
            &identity(seed),
            many_modules::web::RemoveArgs {
                owner: w.owner,
                site_name,
                memo: w.memo.clone(),
            },
        )
        .expect("Website removal failed");
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[then(expr = "the {string} value of website {string} for owner identity {int} is")]
fn then_live(w: &mut World, step: &Step, file: String, site_name: String, seed: u32) {
    let value = step.docstring().expect("Docstring is empty");
    let ret = w
        .module
        .get(
            &identity(0),
            GetArgs {
                key: format!("{}/{}/{}/{}", HTTP_ROOT, identity(seed), site_name, file)
                    .into_bytes()
                    .into(),
            },
        )
        .expect("Website not found");
    assert_eq!(
        ret.value.expect("Key is empty"),
        value.clone().into_bytes().into()
    );
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[then(expr = "{string} of website {string} for identity {int} is empty")]
fn then_empty(w: &mut World, file: String, site_name: String, seed: u32) {
    let ret = w
        .module
        .get(
            &identity(seed),
            GetArgs {
                key: format!("{}/{}/{}/{}", HTTP_ROOT, identity(seed), site_name, file)
                    .into_bytes()
                    .into(),
            },
        )
        .expect("Website not found");
    assert_eq!(ret.value, None);
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[then(expr = "the website list should contain {string}")]
fn then_list(w: &mut World, site_name: String) {
    let ret = WebModuleBackend::list(
        &w.module,
        &identity(0),
        ListArgs {
            count: None,
            order: None,
            filter: None,
        },
    )
    .expect("Website list failed");
    assert!(ret
        .deployments
        .into_iter()
        .any(|v| v.site_name == site_name));
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[then(expr = "the website list filtered by identity {int} should contain {string}")]
fn then_list_filtered(w: &mut World, seed: u32, site_name: String) {
    let ret = WebModuleBackend::list(
        &w.module,
        &identity(0),
        ListArgs {
            count: None,
            order: None,
            filter: Some(vec![WebDeploymentFilter::Owner(identity(seed))]),
        },
    )
    .expect("Website list failed");
    assert!(ret
        .deployments
        .into_iter()
        .any(|v| v.site_name == site_name));
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[then(expr = "listing websites with count set to {int} result in a list of length count")]
fn then_list_count(w: &mut World, count: usize) {
    let ret = WebModuleBackend::list(
        &w.module,
        &identity(0),
        ListArgs {
            count: Some(count),
            order: None,
            filter: None,
        },
    )
    .expect("Website list failed");
    assert_eq!(ret.deployments.len(), count);
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[then(expr = "the website list should not contain {string}")]
fn then_list_not(w: &mut World, site_name: String) {
    let ret = WebModuleBackend::list(
        &w.module,
        &identity(0),
        ListArgs {
            count: None,
            order: None,
            filter: None,
        },
    )
    .expect("Website list failed");
    assert!(ret
        .deployments
        .into_iter()
        .any(|v| v.site_name != site_name));
}

#[allow(clippy::needless_pass_by_ref_mut)]
#[then(expr = "the website list filtered by identity {int} should not contain {string}")]
fn then_list_not_filtered(w: &mut World, seed: u32, site_name: String) {
    let ret = WebModuleBackend::list(
        &w.module,
        &identity(0),
        ListArgs {
            count: None,
            order: None,
            filter: Some(vec![WebDeploymentFilter::Owner(identity(seed))]),
        },
    )
    .expect("Website list failed");
    assert!(ret
        .deployments
        .into_iter()
        .any(|v| v.site_name != site_name));
}

#[then(expr = "the website deployment fails with {string}")]
fn then_deployment_failed(w: &mut World, error: String) {
    assert!(matches!(
        w.module.deploy(
            &identity(0),
            DeployArgs {
                owner: w.owner,
                site_name: w.site_name.clone(),
                site_description: w.site_description.clone(),
                source: w.source.clone(),
                memo: w.memo.clone(),
                domain: w.domain.clone(),
            },
        ),
        Err(e) if e.to_string() == error
    ));
}

#[then(expr = "the website update fails with {string}")]
fn then_update_failed(w: &mut World, error: String) {
    assert!(matches!(
        w.module.update(
            &identity(0),
            UpdateArgs {
                owner: w.owner,
                site_name: w.site_name.clone(),
                site_description: w.site_description.clone(),
                source: w.source.clone(),
                memo: w.memo.clone(),
                domain: w.domain.clone(),
            },
        ),
        Err(e) if e.to_string() == error
    ));
}

#[then(expr = "the website removal fails with {string}")]
fn then_remove_failed(w: &mut World, error: String) {
    assert!(matches!(
        w.module.remove(
            &identity(0),
            many_modules::web::RemoveArgs {
                owner: w.owner,
                site_name: w.site_name.clone(),
                memo: w.memo.clone()
                }
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
