use clap::Parser;
use many_client::client::blocking::ManyClient;
use many_identity::{Address, AnonymousIdentity, Identity};
use many_identity_dsa::CoseKeyIdentity;
use many_modules::kvstore::{GetArgs, GetReturns};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use tracing::{debug, info, warn};

type Client = Arc<ManyClient<Box<dyn Identity>>>;

#[derive(clap::ArgEnum, Clone)]
enum LogStrategy {
    Terminal,
    Syslog,
}

#[derive(Debug, Parser)]
struct Opts {
    #[clap(flatten)]
    common_flags: many_cli_helpers::CommonCliFlags,

    /// Many server URL to connect to. It must implement a KV-Store attribute.
    #[clap(default_value = "http://localhost:8000")]
    server: String,

    /// Port and address to bind to.
    #[clap(long)]
    addr: SocketAddr,

    /// The identity of the server (an identity string), or anonymous if you don't know it.
    server_id: Option<Address>,

    /// A PEM file for the identity. If not specified, anonymous will be used.
    #[clap(long)]
    pem: Option<PathBuf>,

    /// Number of threads to use for request processing. Defaults to 1.
    #[clap(long)]
    #[clap(value_parser = clap::value_parser!(u8).range(1..))]
    num_threads: Option<u8>,
}

fn process_request(http: Arc<Server>, client: Client) -> impl Fn() {
    move || {
        for request in http.incoming_requests() {
            match request.method() {
                Method::Get => handle_get_request(&client, request),
                x => {
                    warn!("Received unknown method: {}", x);
                    let _ = request.respond(Response::empty(StatusCode::from(405)));
                }
            }
        }
    }
}

fn handle_get_request(client: &Client, request: Request) {
    let mut path = "/http".to_string();
    let url = request.url();
    let maybe_host = request.headers().iter().find(|h| h.field.equiv("host"));
    if let Some(host) = maybe_host {
        let parts: Vec<_> = host.value.as_str().splitn(3, '.').collect();
        if let [site_name, addr, _] = parts.as_slice() {
            path = format!("{path}/{addr}/{site_name}")
        }
    }
    debug!("Received request for path: {path}{url}");
    let result = client.call_(
        "kvstore.get",
        GetArgs {
            key: format!("{path}{url}").into_bytes().into(),
        },
    );
    match result {
        Ok(result) => process_result(result, request),
        Err(_) => {
            let _ = request.respond(Response::empty(500));
        }
    }
}

fn process_result(result: Vec<u8>, request: Request) {
    match minicbor::decode::<GetReturns>(&result) {
        Ok(GetReturns { value }) => match value {
            None => {
                if let Err(e) = request.respond(Response::empty(404)) {
                    warn!("Failed to send response: {}", e);
                }
            }
            Some(value) => respond_with_value(value.into(), request),
        },
        Err(e) => {
            warn!("Failed to decode result: {}", e);
            if let Err(e) = request.respond(Response::empty(500)) {
                warn!("Failed to send response: {}", e);
            }
        }
    }
}

fn respond_with_value(value: Vec<u8>, request: Request) {
    let mimetype = new_mime_guess::from_path(request.url()).first_raw();
    let mut response = Response::empty(200).with_data(value.as_slice(), Some(value.len()));

    if let Some(mimetype) = mimetype {
        if let Ok(header) = Header::from_bytes("Content-Type", mimetype) {
            response = response.with_header(header);
        } else {
            warn!("Failed to create header for mimetype: {}", mimetype);
        }
    }

    if let Err(e) = request.respond(response) {
        warn!("Failed to send response: {}", e);
    }
}

fn main() {
    let Opts {
        common_flags,
        addr,
        pem,
        server,
        server_id,
        num_threads,
    } = Opts::parse();

    common_flags.init_logging().unwrap();

    debug!("{:?}", Opts::parse());
    info!(
        version = env!("CARGO_PKG_VERSION"),
        git_sha = env!("VERGEN_GIT_SHA")
    );

    let server_id = server_id.unwrap_or_default();
    let key: Box<dyn Identity> = pem.map_or_else(
        || Box::new(AnonymousIdentity) as Box<dyn Identity>,
        |p| Box::new(CoseKeyIdentity::from_pem(std::fs::read_to_string(p).unwrap()).unwrap()),
    );

    let client = Client::new(ManyClient::new(server, server_id, key).unwrap());
    let http = Arc::new(tiny_http::Server::http(addr).unwrap());

    let mut handles = Vec::new();

    for _ in 0..num_threads.unwrap_or(1) {
        let http = http.clone();
        let client = client.clone();
        handles.push(thread::spawn(process_request(http, client)));
    }

    for h in handles {
        h.join().unwrap();
    }
}
