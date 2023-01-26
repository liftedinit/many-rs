#![feature(used_with_arg)]

use clap::Parser;
use many_identity::verifiers::AnonymousVerifier;
use many_identity::{Address, Identity};
use many_identity_dsa::{CoseKeyIdentity, CoseKeyVerifier};
use many_identity_webauthn::WebAuthnVerifier;
use many_migration::MigrationConfig;
use many_modules::account::features::Feature;
use many_modules::{abci_backend, account, data, events, idstore, ledger};
use many_protocol::ManyUrl;
use many_server::transport::http::HttpServer;
use many_server::ManyServer;
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::level_filters::LevelFilter;
use tracing::{debug, info, warn};

use crate::allow_addrs::AllowAddrsModule;

#[cfg(feature = "webauthn_testing")]
use crate::idstore_webauthn::IdStoreWebAuthnModule;
use crate::json::InitialStateJson;
use crate::migration::MIGRATIONS;
use crate::module::account::AccountFeatureModule;
use module::*;

mod error;
mod json;
mod migration;
mod module;
mod storage;

#[derive(clap::ArgEnum, Clone, Debug)]
enum LogStrategy {
    Terminal,
    Syslog,
}

#[derive(Parser, Debug)]
#[clap(args_override_self(true))]
struct Opts {
    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long, parse(from_occurrences))]
    verbose: i8,

    /// Suppress all output logging. Can be used multiple times to suppress more.
    #[clap(short, long, parse(from_occurrences))]
    quiet: i8,

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

    /// Use given logging strategy
    #[clap(long, arg_enum, default_value_t = LogStrategy::Terminal)]
    logmode: LogStrategy,

    /// Path to a JSON file containing the configurations for the
    /// migrations. Migrations are DISABLED unless this configuration file
    /// is given.
    #[clap(long, short)]
    migrations_config: Option<PathBuf>,

    /// List built-in migrations supported by this binary
    #[clap(long, exclusive = true)]
    list_migrations: bool,

    /// Path to a JSON file containing an array of MANY addresses
    /// Only addresses from this array will be able to execute commands, e.g., send, put, ...
    /// Any addresses will be able to execute queries, e.g., balance, get, ...
    #[clap(long)]
    allow_addrs: Option<PathBuf>,
}

fn main() {
    let Opts {
        verbose,
        quiet,
        pem,
        addr,
        abci,
        mut state,
        persistent,
        clean,
        logmode,
        migrations_config,
        allow_origin,
        allow_addrs,
        list_migrations,
        ..
    } = Opts::parse();

    let verbose_level = 2 + verbose - quiet;
    let log_level = match verbose_level {
        x if x > 3 => LevelFilter::TRACE,
        3 => LevelFilter::DEBUG,
        2 => LevelFilter::INFO,
        1 => LevelFilter::WARN,
        0 => LevelFilter::ERROR,
        x if x < 0 => LevelFilter::OFF,
        _ => unreachable!(),
    };
    let subscriber = tracing_subscriber::fmt::Subscriber::builder().with_max_level(log_level);

    match logmode {
        LogStrategy::Terminal => {
            let subscriber = subscriber.with_writer(std::io::stderr);
            subscriber.init();
        }
        LogStrategy::Syslog => {
            let identity = std::ffi::CStr::from_bytes_with_nul(b"many-ledger\0").unwrap();
            let (options, facility) = Default::default();
            let syslog = syslog_tracing::Syslog::new(identity, options, facility).unwrap();

            let subscriber = subscriber.with_ansi(false).with_writer(syslog);
            subscriber.init();
        }
    };

    debug!("{:?}", Opts::parse());
    info!(
        version = env!("CARGO_PKG_VERSION"),
        git_sha = env!("VERGEN_GIT_SHA")
    );

    if list_migrations {
        for migration in MIGRATIONS {
            println!("Name: {}", migration.name());
            println!("Description: {}", migration.description());
        }
        return;
    }

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

    let state: Option<InitialStateJson> =
        state.map(|p| InitialStateJson::read(p).expect("Could not read state file."));

    info!("Loading migrations from {migrations_config:?}");
    let maybe_migrations = migrations_config.map(|file| {
        let content = std::fs::read_to_string(file)
            .expect("Could not read file passed to --migrations_config");
        let config: MigrationConfig = serde_json::from_str(&content).unwrap();
        config.strict()
    });

    let module_impl = if persistent.exists() {
        if state.is_some() {
            warn!(
                r#"
                An existing persistent store {} was found and a staging file {state:?} was given.
                Ignoring staging file and loading existing persistent store.
                "#,
                persistent.display()
            );
        }

        #[cfg(feature = "balance_testing")]
        {
            let Opts {
                balance_only_for_testing,
                ..
            } = Opts::parse();
            if balance_only_for_testing.is_some() {
                warn!("Loading existing persistent store, ignoring --balance_only_for_testing");
            }
        }

        LedgerModuleImpl::load(maybe_migrations, persistent, abci).unwrap()
    } else if let Some(state) = state {
        #[cfg(feature = "balance_testing")]
        {
            let mut module_impl =
                LedgerModuleImpl::new(state, maybe_migrations, persistent, abci).unwrap();

            use std::str::FromStr;

            let Opts {
                balance_only_for_testing,
                ..
            } = Opts::parse();

            for balance in balance_only_for_testing.unwrap_or_default() {
                let args: Vec<&str> = balance.splitn(3, ':').collect();
                let (identity, amount, symbol) = (
                    args.first().unwrap(),
                    args.get(1).expect("No amount."),
                    args.get(2).expect("No symbol."),
                );

                module_impl
                    .set_balance_only_for_testing(
                        Address::from_str(identity).expect("Invalid identity."),
                        amount.parse::<u64>().expect("Invalid amount."),
                        Address::from_str(symbol).expect("Invalid symbol."),
                    )
                    .expect("Unable to set balance for testing.");
            }
            module_impl
        }

        #[cfg(not(feature = "balance_testing"))]
        LedgerModuleImpl::new(state, maybe_migrations, persistent, abci).unwrap()
    } else {
        panic!("Persistent store or staging file not found.")
    };
    let module_impl = Arc::new(Mutex::new(module_impl));

    let many = ManyServer::simple(
        "many-ledger",
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
        s.add_module(ledger::LedgerModule::new(module_impl.clone()));
        let ledger_command_module = ledger::LedgerCommandsModule::new(module_impl.clone());
        if let Some(path) = allow_addrs {
            let allow_addrs: BTreeSet<Address> =
                json5::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            s.add_module(AllowAddrsModule {
                inner: ledger_command_module,
                allow_addrs,
            });
        } else {
            s.add_module(ledger_command_module);
        }
        s.add_module(events::EventsModule::new(module_impl.clone()));
        s.add_module(ledger::LedgerTokensModule::new(module_impl.clone()));
        s.add_module(ledger::LedgerMintBurnModule::new(module_impl.clone()));

        let idstore_module = idstore::IdStoreModule::new(module_impl.clone());
        #[cfg(feature = "webauthn_testing")]
        {
            let Opts {
                disable_webauthn_only_for_testing,
                ..
            } = Opts::parse();

            if disable_webauthn_only_for_testing {
                s.add_module(IdStoreWebAuthnModule {
                    inner: idstore_module,
                    check_webauthn: false,
                });
            } else {
                s.add_module(idstore_module);
            }
        }
        #[cfg(not(feature = "webauthn_testing"))]
        s.add_module(idstore_module);

        s.add_module(AccountFeatureModule::new(
            account::AccountModule::new(module_impl.clone()),
            [Feature::with_id(0), Feature::with_id(1)],
        ));
        s.add_module(account::features::multisig::AccountMultisigModule::new(
            module_impl.clone(),
        ));
        s.add_module(data::DataModule::new(module_impl.clone()));
        if abci {
            s.set_timeout(u64::MAX);
            s.add_module(abci_backend::AbciModule::new(module_impl));
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
