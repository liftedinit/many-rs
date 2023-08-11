use clap::Parser;
use many_client::client::blocking::ManyClient;
use many_error::ManyError;
use many_identity::{Address, AnonymousIdentity, Identity};
use many_identity_dsa::CoseKeyIdentity;
use many_modules::r#async::{StatusArgs, StatusReturn};
use many_modules::web::ListArgs;
use many_modules::{r#async, web};
use many_protocol::ResponseMessage;
use many_types::web::{WebDeploymentFilter, WebDeploymentSource};
use many_types::{Memo, SortOrder};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, error, info};

#[derive(Debug, Parser)]
struct Opts {
    #[clap(flatten)]
    common_flags: many_cli_helpers::CommonCliFlags,

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

    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Debug, Parser)]
enum SubCommand {
    /// Deploy a new website
    Deploy(DeployOpt),

    /// Remove a website
    Remove(RemoveOpt),

    /// List websites
    List(ListOpt),
}

#[derive(Debug, Parser)]
struct DeployOpt {
    /// Site name
    site_name: String,

    /// Site description
    #[clap(long)]
    site_description: Option<String>,

    /// Site source
    source: PathBuf,

    /// MANY address of the website owner
    #[clap(long)]
    owner: Option<Address>,

    /// A memo to attach to the transaction
    #[clap(long, parse(try_from_str = Memo::try_from))]
    memo: Option<Memo>,
}

#[derive(Debug, Parser)]
struct RemoveOpt {
    /// Site name
    site_name: String,

    /// MANY address of the website owner
    #[clap(long)]
    owner: Option<Address>,

    // A memo to attach to the transaction
    #[clap(long, parse(try_from_str = Memo::try_from))]
    memo: Option<Memo>,
}

#[derive(Debug, Parser)]
struct ListOpt {
    /// Order
    #[clap(long)]
    order: Option<SortOrder>,

    /// Filter
    #[clap(long)]
    filter: Option<Vec<WebDeploymentFilter>>,
}

fn deploy(
    client: ManyClient<impl Identity>,
    site_name: String,
    site_description: Option<String>,
    source: PathBuf,
    owner: Option<Address>,
    memo: Option<Memo>,
) -> Result<(), ManyError> {
    // Read the source file
    let source = std::fs::read(source).map_err(ManyError::unknown)?;
    let arguments = web::DeployArgs {
        owner,
        site_name,
        site_description,
        source: WebDeploymentSource::Zip(source.into()),
        memo,
    };
    let response = client.call("web.deploy", arguments)?;
    let payload = wait_response(client, response)?;
    println!(
        "{}",
        cbor_diag::parse_bytes(&payload).unwrap().to_diag_pretty()
    );
    Ok(())
}

fn remove(
    client: ManyClient<impl Identity>,
    site_name: String,
    owner: Option<Address>,
    memo: Option<Memo>,
) -> Result<(), ManyError> {
    let arguments = web::RemoveArgs {
        owner,
        site_name,
        memo,
    };
    let response = client.call("web.remove", arguments)?;
    let payload = wait_response(client, response)?;
    println!(
        "{}",
        cbor_diag::parse_bytes(&payload).unwrap().to_diag_pretty()
    );
    Ok(())
}

fn list(
    client: ManyClient<impl Identity>,
    order: Option<SortOrder>,
    filter: Option<Vec<WebDeploymentFilter>>,
) -> Result<(), ManyError> {
    let args = ListArgs { order, filter };
    let response = client.call("web.list", args)?;
    let payload = wait_response(client, response)?;
    println!(
        "{}",
        cbor_diag::parse_bytes(&payload).unwrap().to_diag_pretty()
    );
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
        progress.enable_steady_tick(Duration::from_millis(100));

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
        server,
        server_id,
        subcommand,
        common_flags,
        ..
    } = Opts::parse();

    common_flags.init_logging().unwrap();

    debug!("{:?}", Opts::parse());

    let key = pem.map_or_else(
        || Box::new(AnonymousIdentity) as Box<dyn Identity>,
        |p| Box::new(CoseKeyIdentity::from_pem(std::fs::read_to_string(p).unwrap()).unwrap()),
    );

    let client = ManyClient::new(server, server_id, key).unwrap();
    let result = match subcommand {
        SubCommand::Deploy(DeployOpt {
            site_name,
            site_description,
            source,
            owner,
            memo,
        }) => deploy(client, site_name, site_description, source, owner, memo),
        SubCommand::Remove(RemoveOpt {
            site_name,
            owner,
            memo,
        }) => remove(client, site_name, owner, memo),
        SubCommand::List(ListOpt { order, filter }) => list(client, order, filter),
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
