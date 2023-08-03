use std::path::Path;
use cucumber::{given, then, when, World as _};
use tempfile::Builder;
use many_identity::testing::identity;
use many_modules::web::{DeployArgs, WebCommandsModuleBackend};
use many_types::web::WebDeploymentSource;
use many_web::module::{InitialStateJson, WebModuleImpl};

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
                Builder::new().prefix("many-web").tempdir().expect("Unable to create temporary directory"),
                false,
            ).expect("Unable to create web module"),
        }
    }
}

// #[given(expr = "a GitHub source with repo url {string}")]
// fn given_source(w: &mut World, source: String) {
//     match &mut w.source {
//         WebDeploymentSource::Zip(source) => *url = source,
//     }
// }

#[given(expr = "a website name {string}")]
fn given_site_name(w: &mut World, name: String) {
    w.site_name = name;
}

#[given(expr = "a website description {string}")]
fn given_site_description(w: &mut World, description: String) {
        w.site_description = Some(description);
}

#[then(expr = "the website is deployed as identity {int}")]
fn then_deploy(w: &mut World, seed: u32) {
    w.module.deploy(&identity(seed), DeployArgs {
        site_name: w.site_name.clone(),
        site_description: w.site_description.clone(),
        source: w.source.clone(),
    }).expect("Website deployment failed");
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

