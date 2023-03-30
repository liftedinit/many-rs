#![feature(used_with_arg)]

use {
    crate::allow_addrs::AllowAddrsModule,
    clap::Parser,
    derive_more::{From, TryInto},
    many_cli_helpers::CommonCliFlags,
    many_error::ManyError,
    many_identity::verifiers::AnonymousVerifier,
    many_identity::{Address, Identity},
    many_identity_dsa::{CoseKeyIdentity, CoseKeyVerifier},
    many_identity_webauthn::WebAuthnVerifier,
    many_migration::MigrationConfig,
    many_modules::account::features::Feature,
    many_modules::{abci_backend, account, data, events, idstore, ledger},
    many_protocol::ManyUrl,
    many_server::transport::http::HttpServer,
    many_server::ManyServer,
    std::collections::BTreeSet,
    std::net::SocketAddr,
    std::path::PathBuf,
    std::sync::{Arc, Mutex},
    tracing::{debug, info, warn},
};

#[cfg(feature = "webauthn_testing")]
use {
    crate::idstore_webauthn::IdStoreWebAuthnModule, crate::json::InitialStateJson,
    crate::migration::MIGRATIONS, crate::module::account::AccountFeatureModule, module::*,
};

mod error;
mod json;
mod migration;
mod module;
mod storage;

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

#[derive(Debug, From, TryInto)]
enum Error {
    Anyhow(anyhow::Error),
    Io(std::io::Error),
    Json(json5::Error),
    Many(ManyError),
    Message(String),
    ParseInt(std::num::ParseIntError),
    Serde(serde_json::Error),
}

fn main() -> Result<(), Error> {
    let Opts {
        common_flags,
        pem,
        addr,
        abci,
        mut state,
        persistent,
        clean,
        migrations_config,
        allow_origin,
        allow_addrs,
        list_migrations,
        ..
    } = Opts::parse();

    common_flags.init_logging()?;

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
        return Ok(());
    }

    // At this point the Options should contain a value.
    let pem = pem.ok_or("Identity value should be present".to_string())?;
    let persistent = persistent.ok_or("Persistent value should be present".to_string())?;

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

    let pem = std::fs::read_to_string(pem)?;
    let key = CoseKeyIdentity::from_pem(pem)?;
    info!(address = key.address().to_string().as_str());

    let state = state
        .map(|p| InitialStateJson::read(p).map_err(|error| error.to_string()))
        .transpose()?;

    info!("Loading migrations from {migrations_config:?}");
    let maybe_migrations = migrations_config
        .map(|file| {
            let content = std::fs::read_to_string(file)?;
            let config: MigrationConfig = serde_json::from_str(&content)?;
            Ok::<_, Error>(config.strict())
        })
        .transpose()?;

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

        LedgerModuleImpl::load(maybe_migrations, persistent, abci)?
    } else if let Some(state) = state {
        #[cfg(feature = "balance_testing")]
        {
            let mut module_impl = LedgerModuleImpl::new(state, maybe_migrations, persistent, abci)?;

            use std::str::FromStr;

            let Opts {
                balance_only_for_testing,
                ..
            } = Opts::parse();

            for balance in balance_only_for_testing.unwrap_or_default() {
                let args: Vec<&str> = balance.splitn(3, ':').collect();
                let (identity, amount, symbol) = (
                    args.first()
                        .ok_or("Missing arguments for balance testing".to_string())?,
                    args.get(1).ok_or("No amount.".to_string())?,
                    args.get(2).ok_or("No symbol.".to_string())?,
                );

                module_impl.set_balance_only_for_testing(
                    Address::from_str(identity)?,
                    amount.parse::<u64>()?,
                    Address::from_str(symbol)?,
                )?;
            }
            module_impl
        }

        #[cfg(not(feature = "balance_testing"))]
        LedgerModuleImpl::new(state, maybe_migrations, persistent, abci)?
    } else {
        Err("Persistent store or staging file not found.".to_string())?
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
        let mut s = many
            .lock()
            .map_err(|_| "Could not acquire server lock".to_string())?;
        s.add_module(ledger::LedgerModule::new(module_impl.clone()));
        let ledger_command_module = ledger::LedgerCommandsModule::new(module_impl.clone());
        if let Some(path) = allow_addrs {
            let allow_addrs: BTreeSet<Address> = json5::from_str(&std::fs::read_to_string(path)?)?;
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

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(many_server.bind(addr)).map_err(Into::into)
}
