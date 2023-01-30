use clap::Parser;
use many_client::client::blocking::ManyClient;
use many_error::{ManyError, Reason};
use many_identity::{Address, AnonymousIdentity, Identity};
use many_identity_dsa::CoseKeyIdentity;
use many_modules::kvstore::TransferArgs;
use many_modules::r#async::{StatusArgs, StatusReturn};
use many_modules::{kvstore, r#async};
use many_protocol::ResponseMessage;
use many_types::Either;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, error, info};
use tracing_subscriber::filter::LevelFilter;

#[derive(clap::ArgEnum, Clone, Debug)]
enum LogStrategy {
    Terminal,
    Syslog,
}

#[derive(Debug, Parser)]
struct Opts {
    /// Many server URL to connect to.
    #[clap(default_value = "http://localhost:8000")]
    server: String,

    /// The identity of the server (an identity string), or anonymous if you don't know it.
    #[clap(default_value_t)]
    #[clap(long)]
    server_id: Address,

    /// A PEM file for the identity. If not specified, anonymous will be used.
    #[clap(long)]
    pem: Option<PathBuf>,

    /// An alternative owner Address
    #[clap(long)]
    alt_owner: Option<Address>,

    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long, parse(from_occurrences))]
    verbose: i8,

    /// Suppress all output logging. Can be used multiple times to suppress more.
    #[clap(short, long, parse(from_occurrences))]
    quiet: i8,

    /// Use given logging strategy
    #[clap(long, arg_enum, default_value_t = LogStrategy::Terminal)]
    logmode: LogStrategy,

    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Debug, Parser)]
enum SubCommand {
    /// Get a value from the key-value store.
    Get(GetOpt),

    /// Query a key from the key-value store.
    Query(QueryOpt),

    /// Put a value in the store.
    Put(PutOpt),

    /// Disable a value from the store.
    Disable(DisableOpt),

    /// Transfer ownership of a key.
    Transfer(TransferOpt),
}

#[derive(Debug, Parser)]
struct GetOpt {
    /// The key to get.
    key: String,

    /// If the key is passed as an hexadecimal string, pass this key.
    #[clap(long)]
    hex_key: bool,

    /// Whether to output using hexadecimal, or regular value.
    #[clap(long)]
    hex: bool,
}

#[derive(Debug, Parser)]
struct QueryOpt {
    /// The key to get.
    key: String,

    /// If the key is passed as an hexadecimal string, pass this key.
    #[clap(long)]
    hex_key: bool,
}

#[derive(Debug, Parser)]
struct PutOpt {
    /// The key to set.
    key: String,

    /// If the key is a hexadecimal string, pass this flag.
    #[clap(long)]
    hex_key: bool,

    /// The value to set. Use `--stdin` to read the value from STDIN.
    #[clap(conflicts_with = "stdin")]
    value: Option<String>,

    /// Use this flag to use STDIN to get the value.
    #[clap(long, conflicts_with = "value")]
    stdin: bool,
}

#[derive(Debug, Parser)]
struct DisableOpt {
    /// The key to disable.
    key: String,

    /// If the key is a hexadecimal string, pass this flag.
    #[clap(long)]
    hex_key: bool,

    /// Reason for disabling the key
    #[clap(long)]
    reason: Option<String>,
}

#[derive(Debug, Parser)]
struct TransferOpt {
    /// The key to disable.
    key: String,

    /// If the key is passed as an hexadecimal string, pass this key.
    #[clap(long)]
    hex_key: bool,

    /// The new owner of the key to transfer to.
    new_owner: Address,
}

fn get(client: ManyClient<impl Identity>, key: &[u8], hex: bool) -> Result<(), ManyError> {
    let arguments = kvstore::GetArgs {
        key: key.to_vec().into(),
    };

    let payload = client.call_("kvstore.get", arguments)?;
    if payload.is_empty() {
        Err(ManyError::unexpected_empty_response())
    } else {
        let result: kvstore::GetReturns =
            minicbor::decode(&payload).map_err(ManyError::deserialization_error)?;
        let value = result.value;

        if let Some(value) = value {
            if hex {
                println!("{}", hex::encode(value.as_slice()));
            } else {
                std::io::Write::write_all(&mut std::io::stdout(), &value).unwrap();
            }
        } else {
            println!("{value:?}");
        }

        Ok(())
    }
}

fn query(client: ManyClient<impl Identity>, key: &[u8]) -> Result<(), ManyError> {
    let arguments = kvstore::QueryArgs {
        key: key.to_vec().into(),
    };

    let payload = client.call_("kvstore.query", arguments)?;
    if payload.is_empty() {
        Err(ManyError::unexpected_empty_response())
    } else {
        let result: kvstore::QueryReturns =
            minicbor::decode(&payload).map_err(ManyError::deserialization_error)?;

        let owner = result.owner.to_string();

        match result.disabled {
            Some(Either::Left(true)) => println!("{owner}, disabled"),
            Some(Either::Right(reason)) => println!("{owner}, disabled ({reason})"),
            _ => println!("{owner}"),
        }

        Ok(())
    }
}

fn put(
    client: ManyClient<impl Identity>,
    alt_owner: Option<Address>,
    key: &[u8],
    value: Vec<u8>,
) -> Result<(), ManyError> {
    let arguments = kvstore::PutArgs {
        key: key.to_vec().into(),
        value: value.into(),
        alternative_owner: alt_owner,
    };

    let response = client.call("kvstore.put", arguments)?;
    let payload = wait_response(client, response)?;
    println!("{}", minicbor::display(&payload));
    Ok(())
}

fn disable(
    client: ManyClient<impl Identity>,
    alt_owner: Option<Address>,
    key: &[u8],
    reason: Option<Reason<u64>>,
) -> Result<(), ManyError> {
    let arguments = kvstore::DisableArgs {
        key: key.to_vec().into(),
        alternative_owner: alt_owner,
        reason,
    };

    let response = client.call("kvstore.disable", arguments)?;
    let payload = wait_response(client, response)?;
    println!("{}", minicbor::display(&payload));
    Ok(())
}

fn transfer(
    client: ManyClient<impl Identity>,
    alt_owner: Option<Address>,
    key: Vec<u8>,
    new_owner: Address,
) -> Result<(), ManyError> {
    let args = TransferArgs {
        key: key.into(),
        alternative_owner: alt_owner,
        new_owner,
    };

    let response = client.call("kvstore.transfer", args)?;
    let payload = wait_response(client, response)?;
    println!("{}", minicbor::display(&payload));
    Ok(())
}

pub(crate) fn wait_response(
    client: ManyClient<impl Identity>,
    response: ResponseMessage,
) -> Result<Vec<u8>, ManyError> {
    let ResponseMessage {
        data, attributes, ..
    } = response;

    let payload = data?;
    debug!("response: {}", hex::encode(&payload));
    if payload.is_empty() {
        let attr = match attributes.get::<r#async::attributes::AsyncAttribute>() {
            Ok(attr) => attr,
            _ => {
                info!("Empty payload.");
                return Ok(Vec::new());
            }
        };
        info!("Async token: {}", hex::encode(&attr.token));

        let progress =
            indicatif::ProgressBar::new_spinner().with_message("Waiting for async response");
        progress.enable_steady_tick(100);

        // TODO: improve on this by using duration and thread and watchdog.
        // Wait for the server for ~60 seconds by pinging it every second.
        for _ in 0..60 {
            let response = client.call(
                "async.status",
                StatusArgs {
                    token: attr.token.clone(),
                },
            )?;
            let status: StatusReturn =
                minicbor::decode(&response.data?).map_err(ManyError::deserialization_error)?;
            match status {
                StatusReturn::Done { response } => {
                    progress.finish();
                    let response: ResponseMessage =
                        minicbor::decode(&response.payload.ok_or_else(|| {
                            ManyError::deserialization_error(
                                "Empty payload. Expected ResponseMessage.",
                            )
                        })?)
                        .map_err(ManyError::deserialization_error)?;
                    return wait_response(client, response);
                }
                StatusReturn::Expired => {
                    progress.finish();
                    info!("Async token expired before we could check it.");
                    return Ok(Vec::new());
                }
                _ => {
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
        Err(ManyError::unknown(
            "Transport timed out waiting for async result.",
        ))
    } else {
        Ok(payload)
    }
}

fn main() {
    let Opts {
        pem,
        alt_owner,
        server,
        server_id,
        subcommand,
        verbose,
        quiet,
        logmode,
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
            let identity = std::ffi::CStr::from_bytes_with_nul(b"kvstore\0").unwrap();
            let (options, facility) = Default::default();
            let syslog = syslog_tracing::Syslog::new(identity, options, facility).unwrap();

            let subscriber = subscriber.with_ansi(false).with_writer(syslog);
            subscriber.init();
            log_panics::init();
        }
    };

    debug!("{:?}", Opts::parse());

    let key = pem.map_or_else(
        || Box::new(AnonymousIdentity) as Box<dyn Identity>,
        |p| Box::new(CoseKeyIdentity::from_pem(std::fs::read_to_string(p).unwrap()).unwrap()),
    );

    let client = ManyClient::new(server, server_id, key).unwrap();
    let result = match subcommand {
        SubCommand::Get(GetOpt { key, hex_key, hex }) => {
            let key = if hex_key {
                hex::decode(&key).unwrap()
            } else {
                key.into_bytes()
            };
            get(client, &key, hex)
        }
        SubCommand::Query(QueryOpt { key, hex_key }) => {
            let key = if hex_key {
                hex::decode(&key).unwrap()
            } else {
                key.into_bytes()
            };
            query(client, &key)
        }
        SubCommand::Put(PutOpt {
            key,
            hex_key,
            value,
            stdin,
        }) => {
            let key = if hex_key {
                hex::decode(&key).unwrap()
            } else {
                key.into_bytes()
            };
            let value = if stdin {
                let mut value = Vec::new();
                std::io::stdin().read_to_end(&mut value).unwrap();
                value
            } else {
                value.expect("Must pass a value").into_bytes()
            };
            put(client, alt_owner, &key, value)
        }
        SubCommand::Disable(DisableOpt {
            key,
            hex_key,
            reason,
        }) => {
            let key = if hex_key {
                hex::decode(&key).unwrap()
            } else {
                key.into_bytes()
            };
            let reason = reason.map(|reason| Reason::new(123456, Some(reason), BTreeMap::new()));
            disable(client, alt_owner, &key, reason)
        }
        SubCommand::Transfer(TransferOpt {
            key,
            hex_key,
            new_owner,
        }) => {
            let key = if hex_key {
                hex::decode(&key).unwrap()
            } else {
                key.into_bytes()
            };
            transfer(client, alt_owner, key, new_owner)
        }
    };

    if let Err(err) = result {
        error!(
            "Error returned by server:\n|  {}\n",
            err.to_string()
                .split('\n')
                .collect::<Vec<&str>>()
                .join("\n|  ")
        );
        std::process::exit(1);
    }
}
