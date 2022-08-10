use std::{
    convert::Infallible,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    thread,
};

use async_trait::async_trait;
use cucumber::{given, then, WorldInit};
use many_client::ManyClient;
use many_identity::{Address, CoseKeyIdentity};
use many_mock::server::ManyMockServer;
use many_server::{transport::http::HttpServer, ManyServer};
use serde_json::Value;

#[derive(Debug, WorldInit)]
struct World {
    finish_server: Arc<AtomicBool>,
    client: ManyClient,
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
        let mut mockfile = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        mockfile.push("tests");
        mockfile.push("testmockfile.toml");
        let mockfile = mockfile.as_os_str().to_str().unwrap();

        let mocktree = many_mock::parse_mockfile(mockfile).unwrap();
        let key = CoseKeyIdentity::anonymous();

        let many = ManyServer::simple("integration", key.clone(), None, None);
        {
            let mut many = many.lock().unwrap();
            let mock_server = ManyMockServer::new(mocktree, None, key.clone());
            many.set_fallback_module(mock_server);
        }
        let mut server = HttpServer::new(many);

        let finish_server = server.term_signal();
        thread::spawn(move || {
            server.bind("0.0.0.0:8000").unwrap();
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
    let result = w.client.call(method, ()).unwrap();
    let json_string =
        String::from_utf8(result.data.unwrap()).expect("Should be a valid UTF-8 string");
    let response = serde_json::from_str(&json_string).expect("Should parse to a JSON value");
    w.response = Some(response);
}

#[then(regex = "it should be (.*)")]
async fn full_value(w: &mut World, value: String) {
    let json_value: Value = serde_json::from_str(&value).unwrap();
    assert_eq!(w.response, Some(json_value));
}

#[then(regex = r#""(.*)" should be (.*)"#)]
async fn field_value(w: &mut World, field_name: String, value: String) {
    let object = w
        .response
        .as_mut()
        .unwrap()
        .as_object()
        .expect("Response should be a JSON");
    let json_value: Value = serde_json::from_str(&value).unwrap();
    assert_eq!(object[&field_name], json_value);
}

fn main() {
    futures::executor::block_on(World::run("tests/features"));
}
