use clap::Parser;
use many::message::{encode_cose_sign1_from_request, RequestMessage, RequestMessageBuilder};
use many::server::module::ledger;
use many::transport::http::HttpServer;
use many::types::identity::CoseKeyIdentity;
use many::{Identity, ManyServer};
use many_client::ManyClient;
use std::convert::TryFrom;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process;
use tracing::level_filters::LevelFilter;

#[derive(Parser)]
struct Opts {
    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long, parse(from_occurrences))]
    verbose: i8,

    /// Suppress all output logging. Can be used multiple times to suppress more.
    #[clap(short, long, parse(from_occurrences))]
    quiet: i8,

    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Parser)]
enum SubCommand {
    /// Transform a textual ID into its hexadecimal value, or the other way around.
    /// If the argument is neither hexadecimal value or identity, try to see if it's
    /// a file, and will parse it as a PEM file.
    Id(IdOpt),

    /// Creates a message and output it.
    Message(MessageOpt),

    /// Starts a base server that can also be used for reverse proxying
    /// to another MANY server.
    Server(ServerOpt),

    /// Get the token ID per string of a ledger's token.
    GetTokenId(GetTokenIdOpt),
}

#[derive(Parser)]
struct IdOpt {
    /// An hexadecimal value to encode, or an identity textual format to decode.
    arg: String,

    /// If the argument is a public key hexadecimal, allow to generate the
    /// identity with a specific subresource ID.
    subid: Option<u32>,
}

#[derive(Parser)]
struct MessageOpt {
    /// A pem file to sign the message. If this is omitted, the message will be anonymous.
    #[clap(long)]
    pem: Option<PathBuf>,

    /// Timestamp.
    #[clap(long)]
    timestamp: Option<String>,

    /// If true, prints out the hex value of the message bytes.
    #[clap(long, conflicts_with("base64"))]
    hex: bool,

    /// If true, prints out the base64 value of the message bytes.
    #[clap(long, conflicts_with("hex"))]
    base64: bool,

    /// The identity to send it to.
    #[clap(long)]
    to: Option<Identity>,

    /// The server to connect to.
    #[clap(long)]
    server: Option<url::Url>,

    /// The method to call.
    method: String,

    /// The content of the message itself (its payload).
    data: Option<String>,
}

#[derive(Parser)]
struct ServerOpt {
    /// The location of a PEM file for the identity of this server.
    #[clap(long)]
    pem: PathBuf,

    /// The address and port to bind to for the MANY Http server.
    #[clap(long, short, default_value = "127.0.0.1:8000")]
    addr: SocketAddr,
}

#[derive(Parser)]
struct GetTokenIdOpt {
    /// The server to call. It MUST implement the ledger attribute (2).
    server: url::Url,

    /// The token to get. If not listed in the list of tokens, this will
    /// error.
    symbol: String,
}

fn main() {
    let Opts {
        verbose,
        quiet,
        subcommand,
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
    tracing_subscriber::fmt().with_max_level(log_level).init();

    match subcommand {
        SubCommand::Id(o) => {
            if let Ok(data) = hex::decode(&o.arg) {
                match Identity::try_from(data.as_slice()) {
                    Ok(mut i) => {
                        if let Some(subid) = o.subid {
                            i = i.with_subresource_id(subid);
                        }
                        println!("{}", i)
                    }
                    Err(e) => {
                        eprintln!("Identity did not parse: {:?}", e.to_string());
                        std::process::exit(1);
                    }
                }
            } else if let Ok(mut i) = Identity::try_from(o.arg.clone()) {
                if let Some(subid) = o.subid {
                    i = i.with_subresource_id(subid);
                }
                println!("{}", hex::encode(&i.to_vec()));
            } else if let Ok(pem_content) = std::fs::read_to_string(&o.arg) {
                // Create the identity from the public key hash.
                let mut i = CoseKeyIdentity::from_pem(&pem_content).unwrap().identity;
                if let Some(subid) = o.subid {
                    i = i.with_subresource_id(subid);
                }

                println!("{}", i);
            } else {
                eprintln!("Could not understand the argument.");
                std::process::exit(2);
            }
        }
        SubCommand::Message(o) => {
            // If `pem` is not provided, use anonymous and don't sign.
            let key = o.pem.map_or_else(CoseKeyIdentity::anonymous, |p| {
                CoseKeyIdentity::from_pem(&std::fs::read_to_string(&p).unwrap()).unwrap()
            });
            let from_identity = key.identity;
            let to_identity = o.to.unwrap_or_default();

            let data = o
                .data
                .map_or(vec![], |d| cbor_diag::parse_diag(&d).unwrap().to_bytes());

            if let Some(s) = o.server {
                let client = ManyClient::new(s, to_identity, key).unwrap();
                let response = client.call_raw(o.method, &data).unwrap();

                match &response.data {
                    Ok(payload) => {
                        if payload.is_empty() {
                            eprintln!("Empty response:\n{:#?}", response);
                        } else {
                            println!(
                                "{}",
                                cbor_diag::parse_bytes(&payload).unwrap().to_diag_pretty()
                            );
                        }
                        std::process::exit(0);
                    }
                    Err(err) => {
                        eprintln!(
                            "Error returned by server:\n|  {}\n",
                            err.to_string()
                                .split('\n')
                                .collect::<Vec<&str>>()
                                .join("\n|  ")
                        );
                        std::process::exit(1);
                    }
                }
            } else {
                let message: RequestMessage = RequestMessageBuilder::default()
                    .version(1)
                    .from(from_identity)
                    .to(to_identity)
                    .method(o.method)
                    .data(data)
                    .build()
                    .unwrap();

                let cose = encode_cose_sign1_from_request(message, &key).unwrap();
                let bytes = cose.to_bytes().unwrap();
                if o.hex {
                    println!("{}", hex::encode(&bytes));
                } else if o.base64 {
                    println!("{}", base64::encode(&bytes));
                } else {
                    panic!("Must specify one of hex, base64 or server...");
                }
            }
        }
        SubCommand::Server(o) => {
            let pem = std::fs::read_to_string(&o.pem).expect("Could not read PEM file.");
            let key = CoseKeyIdentity::from_pem(&pem)
                .expect("Could not generate identity from PEM file.");

            let many = ManyServer::simple(
                "many-server",
                key,
                Some(std::env!("CARGO_PKG_VERSION").to_string()),
            );
            HttpServer::new(many).bind(o.addr).unwrap();
        }
        SubCommand::GetTokenId(o) => {
            let client = ManyClient::new(
                o.server,
                Identity::anonymous(),
                CoseKeyIdentity::anonymous(),
            )
            .expect("Could not create a client");
            let status = client.status().expect("Cannot get status of server");

            if !status.attributes.contains(&ledger::LEDGER_MODULE_ATTRIBUTE) {
                eprintln!("Server does not implement Ledger Attribute.");
                process::exit(1);
            }

            let info: ledger::InfoReturns = minicbor::decode(
                &client
                    .call("ledger.info", ledger::InfoArgs)
                    .unwrap()
                    .data
                    .expect("An error happened during the call to ledger.info"),
            )
            .expect("Invalid data returned by server; not CBOR");

            let symbol = o.symbol;
            let id = info
                .local_names
                .into_iter()
                .find(|(_, y)| y == &symbol)
                .map(|(x, _)| x)
                .ok_or_else(|| format!("Could not resolve symbol '{}'", &symbol))
                .unwrap();

            println!("{}", id);
        }
    }
}
