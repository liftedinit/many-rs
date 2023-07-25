use clap::Parser;
use many_cli_helpers::CommonCliFlags;
use many_identity::verifiers::AnonymousVerifier;
use many_identity::{Address, Identity};
use many_identity_dsa::{CoseKeyIdentity, CoseKeyVerifier};
use many_identity_webauthn::WebAuthnVerifier;
use many_modules::{abci_backend, kvstore, web};
use many_protocol::ManyUrl;
use many_server::transport::http::HttpServer;
use many_server::ManyServer;
use many_server_cache::{RequestCacheValidator, RocksDbCacheBackend};
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

mod error;
mod module;
mod storage;

use crate::module::allow_addrs::AllowAddrsModule;
use module::*;

#[derive(Parser, Debug)]
#[clap(args_override_self(true))]
struct Opts {
    #[clap(flatten)]
    common_flags: CommonCliFlags,

    /// The location of a PEM file for the identity of this server.
    // The field needs to be an Option for the clap derive to work properly.
    #[clap(long, required = true)]
    pem: Option<PathBuf>,

    /// The address and port to bind to for the MANY Http server.
    #[clap(long, short, default_value = "127.0.0.1:8000")]
    addr: SocketAddr,

    /// Uses an ABCI application module.
    #[clap(long)]
    abci: bool,

    /// Path of a state file (that will be used for the initial setup).
    #[clap(long)]
    state: Option<PathBuf>,

    /// Path to a persistent store database (rocksdb).
    // The field needs to be an Option for the clap derive to work properly.
    #[clap(long, required = true)]
    persistent: Option<PathBuf>,

    /// Delete the persistent storage to start from a clean state.
    /// If this is not specified the initial state will not be used.
    #[clap(long, short)]
    clean: bool,

    /// Application absolute URLs allowed to communicate with this server. Any
    /// application will be able to communicate with this server if left empty.
    /// Multiple occurences of this argument can be given.
    #[clap(long)]
    allow_origin: Option<Vec<ManyUrl>>,

    /// A list of initial balances. This will be in addition to the genesis
    /// state file in --state and should only be used for testing.
    /// Each transaction MUST be of the format:
    ///     --balance-only-for-testing=<account_address>:<balance>:<symbol_address>
    /// The hashing of the state will not include these.
    /// This requires the feature "balance_testing" to be enabled.
    #[cfg(feature = "balance_testing")]
    #[clap(long)]
    balance_only_for_testing: Option<Vec<String>>,

    /// If set, this flag will disable any validation for webauthn tokens
    /// to access the id store. WebAuthn signatures are still validated.
    /// This requires the feature "webauthn_testing" to be enabled.
    #[cfg(feature = "webauthn_testing")]
    #[clap(long)]
    disable_webauthn_only_for_testing: bool,

    /// Path to a JSON file containing an array of MANY addresses
    /// Only addresses from this array will be able to execute commands, e.g., send, put, ...
    /// Any addresses will be able to execute queries, e.g., balance, get, ...
    #[clap(long)]
    allow_addrs: Option<PathBuf>,

    /// Database path to the request cache to validate duplicate messages.
    /// If unspecified, the server will not verify transactions for duplicate
    /// messages.
    #[clap(long)]
    cache_db: Option<PathBuf>,
}

fn main() {
    let Opts {
        common_flags,
        pem,
        addr,
        abci,
        mut state,
        persistent,
        clean,
        allow_origin,
        allow_addrs,
        cache_db,
        ..
    } = Opts::parse();

    common_flags.init_logging().unwrap();

    debug!("{:?}", Opts::parse());
    info!(
        version = env!("CARGO_PKG_VERSION"),
        git_sha = env!("VERGEN_GIT_SHA")
    );

    // Safe unwrap.
    // At this point the Options should contain a value.
    let pem = pem.unwrap();
    let persistent = persistent.unwrap();

    if clean {
        // Delete the persistent storage.
        // Ignore NotFound errors.
        match std::fs::remove_dir_all(persistent.as_path()) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                panic!("Error: {e}")
            }
        }
    } else if persistent.exists() {
        // Initial state is ignored.
        state = None;
    }

    let pem = std::fs::read_to_string(pem).expect("Could not read PEM file.");
    let key = CoseKeyIdentity::from_pem(pem).expect("Could not generate identity from PEM file.");
    info!(address = key.address().to_string().as_str());

    let state = state.map(|state| {
        let content = std::fs::read_to_string(state).unwrap();
        json5::from_str(&content).unwrap()
    });

    let module = if persistent.exists() {
        if state.is_some() {
            tracing::warn!(
                r#"
                An existing persistent store {} was found and a staging file {state:?} was given.
                Ignoring staging file and loading existing persistent store.
                "#,
                persistent.display()
            );
        }

        WebModuleImpl::load(persistent, abci).unwrap()
    } else if let Some(state) = state {
        WebModuleImpl::new(state, persistent, abci).unwrap()
    } else {
        panic!("Persistent store or staging file not found.")
    };

    let module = Arc::new(Mutex::new(module));

    let many = ManyServer::simple(
        "many-web",
        key,
        (
            AnonymousVerifier,
            CoseKeyVerifier,
            WebAuthnVerifier::new(allow_origin),
        ),
        Some(env!("CARGO_PKG_VERSION").to_string()),
    );

    {
        let mut s = many.lock().unwrap();
        let web_module = web::WebModule::new(module.clone());
        if let Some(path) = allow_addrs {
            // FIXME: Filter both queries and commands for now
            let allow_addrs: BTreeSet<Address> =
                json5::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            s.add_module(AllowAddrsModule {
                inner: web_module,
                allow_addrs,
            });
        } else {
            s.add_module(web_module);
        }
        // FIXME: Activate and impl those
        s.add_module(kvstore::KvStoreModule::new(module.clone()));
        // s.add_module(events::EventsModule::new(module.clone()));

        if abci {
            s.set_timeout(u64::MAX);
            s.add_module(abci_backend::AbciModule::new(module));
        }

        if let Some(p) = cache_db {
            s.add_validator(RequestCacheValidator::new(RocksDbCacheBackend::new(p)));
        }
    }
    let mut many_server = HttpServer::new(many);

    signal_hook::flag::register(signal_hook::consts::SIGTERM, many_server.term_signal())
        .expect("Could not register signal handler");
    signal_hook::flag::register(signal_hook::consts::SIGHUP, many_server.term_signal())
        .expect("Could not register signal handler");
    signal_hook::flag::register(signal_hook::consts::SIGINT, many_server.term_signal())
        .expect("Could not register signal handler");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(many_server.bind(addr)).unwrap();
}
