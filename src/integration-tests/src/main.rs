use many_client::ManyClient;
use many_identity::CoseKeyIdentity;
use many_server::{transport::http::HttpServer, Address, ManyServer};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    thread,
};

#[derive(Clone, Debug)]
struct Environment {
    finish_server: Arc<AtomicBool>,
    client: ManyClient,
}

fn main() {
    let mut mockfile = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    mockfile.push("testmockfile.toml");

    // parse_mockfile needs the variable to be a &str
    let mockfile = mockfile.as_os_str().to_str().unwrap();

    let mocktree = many_mock::parse_mockfile(mockfile).unwrap();
    let key = CoseKeyIdentity::anonymous();

    let many = ManyServer::simple("test", key.clone(), mocktree, None, None);
    let mut server = HttpServer::new(many);
    let finish_server = server.term_signal();
    // Start test server in a different thread
    thread::spawn(move || {
        server.bind("0.0.0.0:8000").unwrap();
    });
    let address = Address::anonymous();
    let client = ManyClient::new("http://0.0.0.0:8000/", address, key).unwrap();
    let environment = Environment {
        finish_server,
        client,
    };

    rspec::run(&rspec::suite("Integration tests", environment, |suite| {
        suite.context("Mock API", |ctx| {
            ctx.example("should answer with a JSON", |ex| {
                let result = ex.client.call("object", ()).unwrap();
                let json_string = String::from_utf8(result.data.unwrap())
                    .expect("Should be a valid UTF-8 string");
                let json: serde_json::Value =
                    serde_json::from_str(&json_string).expect("Should parse to a JSON Value");
                let object = json.as_object().expect("Response should be a JSON");
                assert_eq!(object["numfield"], 10);
                let arrayfield: Vec<&str> = object["arrayfield"]
                    .as_array()
                    .expect("arrayfield should be an array")
                    .iter()
                    .map(|x| x.as_str().unwrap())
                    .collect();
                assert_eq!(arrayfield, ["foo", "bar", "baz"]);
            });

            ctx.example("should answer with a string", |ex| {
                let result = ex.client.call("simplefield", ()).unwrap();
                let json_string = String::from_utf8(result.data.unwrap())
                    .expect("Should be a valid UTF-8 string");
                let json: serde_json::Value =
                    serde_json::from_str(&json_string).expect("Should parse to a JSON Value");
                let simplefield = json.as_str().expect("Response should be a JSON string");
                assert_eq!(simplefield, "hello");
            });

            ctx.after_all(|ctx| {
                ctx.finish_server
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            });
        });
    }));
}
