use std::{
    collections::BTreeMap,
    convert::Infallible,
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

use async_trait::async_trait;
use ciborium::value::Value;
use cucumber::{given, then, WorldInit};
use many_client::ManyClient;
use many_identity::{AcceptAllVerifier, Address, AnonymousIdentity};
use many_mock::server::ManyMockServer;
use many_server::{transport::http::HttpServer, ManyServer};

#[derive(Debug, WorldInit)]
struct World {
    finish_server: Arc<AtomicBool>,
    client: ManyClient<AnonymousIdentity>,
    response: Option<Value>,
}

impl Drop for World {
    fn drop(&mut self) {
        self.finish_server
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[async_trait(?Send)]
impl cucumber::World for World {
    type Error = Infallible;

    async fn new() -> Result<Self, Self::Error> {
        // Support both Cargo and Bazel paths
        let tmp = format!("{}/tests/testmockfile.toml", env!("CARGO_MANIFEST_DIR"));
        let mockfile = [tmp.as_ref(), "src/many-mock/tests/testmockfile.toml"]
            .into_iter()
            .find(|&p| Path::new(p).exists())
            .expect("Test mock file not found");

        let mocktree = many_mock::parse_mockfile(mockfile).unwrap();
        let key = AnonymousIdentity;

        let many = ManyServer::simple("integration", key.clone(), AcceptAllVerifier, None);
        {
            let mut many = many.lock().unwrap();
            let mock_server = ManyMockServer::new(mocktree, None, key.clone());
            many.set_fallback_module(mock_server);
        }
        let mut server = HttpServer::new(many);

        let finish_server = server.term_signal();
        tokio::task::spawn(async move {
            server.bind("0.0.0.0:8000").await.unwrap();
        });

        let address = Address::anonymous();
        let client = ManyClient::new("http://0.0.0.0:8000/", address, key).unwrap();

        Ok(World {
            finish_server,
            client,
            response: None,
        })
    }
}

#[given(regex = r#"I request "(.*)""#)]
async fn make_request(w: &mut World, method: String) {
    let result = w.client.call(method, ()).await.unwrap();
    let bytes = result.data.expect("Should have a Vec<u8>");
    let response: Value =
        ciborium::de::from_reader(bytes.as_slice()).expect("Should have parsed to a cbor value");
    w.response = Some(response);
}

#[then(regex = "it should be (.*)")]
async fn full_value(w: &mut World, value: String) {
    let json_value: Value = serde_json::from_str(&value).unwrap();
    assert_eq!(w.response, Some(json_value));
}

#[then(regex = r#""(.*)" should be (.*)"#)]
async fn field_value(w: &mut World, field_name: String, value: String) {
    let object: BTreeMap<String, Value> = w
        .response
        .as_ref()
        .unwrap()
        .as_map()
        .expect("Response should be a CBOR")
        .iter()
        .map(|(k, v)| (k.as_text().unwrap().to_string(), v.clone()))
        .collect();
    let json_value: Value = serde_json::from_str(&value).unwrap();
    assert_eq!(object[&field_name], json_value);
}

#[tokio::main]
async fn main() {
    // Support both Cargo and Bazel paths
    let features = ["tests/features", "src/many-mock/tests/features"]
        .into_iter()
        .find(|&p| Path::new(p).exists())
        .expect("Cucumber test features not found");
    World::run(features).await;
}
