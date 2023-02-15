mod delegation;

use anyhow::anyhow;
use async_recursion::async_recursion;
use clap::{ArgGroup, Parser};
use coset::{CborSerializable, CoseSign1};
use many_client::ManyClient;
use many_error::ManyError;
use many_identity::verifiers::AnonymousVerifier;
use many_identity::{Address, AnonymousIdentity, Identity};
use many_identity_dsa::{CoseKeyIdentity, CoseKeyVerifier};
use many_identity_hsm::{Hsm, HsmIdentity, HsmMechanismType, HsmSessionType, HsmUserType};
use many_identity_webauthn::WebAuthnIdentity;
use many_mock::{parse_mockfile, server::ManyMockServer, MockEntries};
use many_modules::delegation::attributes::DelegationAttribute;
use many_modules::delegation::DelegationModuleBackend;
use many_modules::r#async::attributes::AsyncAttribute;
use many_modules::r#async::{StatusArgs, StatusReturn};
use many_modules::{idstore, ledger};
use many_protocol::{
    encode_cose_sign1_from_request, BaseIdentityResolver, ManyUrl, RequestMessage,
    RequestMessageBuilder, ResponseMessage,
};
use many_server::transport::http::HttpServer;
use many_server::ManyServer;
use many_types::Timestamp;
use std::convert::TryFrom;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::{process, sync};
use tracing::{debug, error, info, trace};
use url::Url;

#[derive(Parser)]
struct Opts {
    #[clap(flatten)]
    verbosity: many_cli_helpers::Verbosity,

    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Parser)]
enum SubCommand {
    /// Transform a textual ID into its hexadecimal value, or the other way around.
    /// If the argument is neither hexadecimal value or identity, try to see if it's
    /// a file, and will parse it as a PEM file.
    Id(IdOpt),

    /// Display the textual ID of a public key located on an HSM.
    HsmId(HsmIdOpt),

    /// Display the textual ID of a webauthn key.
    WebauthnId(WebauthnIdOpt),

    /// Creates a message and output it.
    Message(Box<MessageOpt>),

    /// Starts a base server that can also be used for reverse proxying
    /// to another MANY server.
    Server(ServerOpt),

    /// Get the token ID per string of a ledger's token.
    GetTokenId(GetTokenIdOpt),

    /// Manage delegation certificates.
    Delegation(delegation::DelegationOpt),
}

#[derive(Parser)]
struct IdOpt {
    /// An hexadecimal value to encode, an identity textual format to decode or
    /// a PEM file to read
    arg: String,

    /// Allow to generate the identity with a specific subresource ID.
    subid: Option<u32>,
}

#[derive(Parser)]
struct HsmIdOpt {
    /// HSM PKCS#11 module path
    module: PathBuf,

    /// HSM PKCS#11 slot ID
    slot: u64,

    /// HSM PKCS#11 key ID
    keyid: String,

    /// Allow to generate the identity with a specific subresource ID.
    subid: Option<u32>,
}

#[derive(Parser)]
struct WebauthnIdOpt {
    /// URL to the relying party (the MANY server implementing idstore).
    rp: ManyUrl,

    /// The recall phrase.
    #[clap(long, conflicts_with("address"))]
    phrase: Option<String>,

    /// The address of the webauthn key. This may seem redundant but in this
    /// case the webauthn flow will still be checked to get the ID.
    #[clap(long, conflicts_with("phrase"))]
    address: Option<Address>,
}

#[derive(Parser)]
#[clap(
    group(
        ArgGroup::new("hsm")
            .multiple(true)
            .args(&["module", "slot", "keyid"])
            .requires_all(&["module", "slot", "keyid"])
    ),
    group(
        ArgGroup::new("action")
            .args(&["server", "hex", "base64"])
            .required(true)
    )
)]
struct MessageOpt {
    /// A pem file to sign the message. If this is omitted, the message will be anonymous.
    #[clap(long)]
    pem: Option<PathBuf>,

    /// An optional delegation certificate list to use for the message.
    #[clap(long)]
    delegation: Option<Vec<PathBuf>>,

    /// Use Webauthn as the authentication scheme.
    #[clap(long, conflicts_with("pem"))]
    webauthn: bool,

    /// The origin to use in the webauthn flow. By default will use the
    /// relying party's protocol, hostname and port.
    #[clap(long, requires("webauthn"))]
    webauthn_origin: Option<ManyUrl>,

    /// The Webauthn provider. By default will use the same server.
    #[clap(long, requires("webauthn"))]
    rp: Option<ManyUrl>,

    /// The recall phrase for webauthn.
    #[clap(long, requires("webauthn"), conflicts_with("address"))]
    phrase: Option<String>,

    /// The address for webauthn.
    #[clap(long, requires("webauthn"), conflicts_with("phrase"))]
    address: Option<Address>,

    /// The Relaying party Identifier. A string which was used when creating
    /// the credentials.
    /// By default, this will be the hostname of the origin URL, whichever
    /// it is.
    #[clap(long, requires("webauthn"))]
    rp_id: Option<String>,

    /// Timestamp (in seconds since epoch).
    #[clap(long)]
    timestamp: Option<u64>,

    /// The server to connect to.
    #[clap(long)]
    server: Option<Url>,

    /// If true, prints out the hex value of the message bytes.
    #[clap(long)]
    hex: bool,

    /// If true, prints out the base64 value of the message bytes.
    #[clap(long)]
    base64: bool,

    /// If used, send the message from hexadecimal to the server and wait for
    /// the response.
    #[clap(long, requires("server"))]
    from_hex: Option<String>,

    /// Show the async token and exit right away. By default, will poll for the
    /// result of the async operation.
    #[clap(long)]
    r#async: bool,

    /// The identity to send it to.
    #[clap(long)]
    to: Option<Address>,

    /// HSM PKCS#11 module path
    #[clap(long, conflicts_with("pem"))]
    module: Option<PathBuf>,

    /// HSM PKCS#11 slot ID
    #[clap(long, conflicts_with("pem"))]
    slot: Option<u64>,

    /// HSM PKCS#11 key ID
    #[clap(long, conflicts_with("pem"))]
    keyid: Option<String>,

    /// The method to call.
    method: Option<String>,

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

    /// The name to give the server.
    #[clap(long, short, default_value = "many-server")]
    name: String,

    /// The path to a mockfile containing mock responses.
    /// Default is mockfile.toml, gives an error if the file does not exist
    #[clap(long, short, value_parser = parse_mockfile)]
    mockfile: Option<MockEntries>,

    /// Add support for delegation.
    #[clap(long)]
    delegation: bool,
}

#[derive(Parser)]
struct GetTokenIdOpt {
    /// The server to call. It MUST implement the ledger attribute (2).
    server: Url,

    /// The token to get. If not listed in the list of tokens, this will
    /// error.
    symbol: String,
}

#[async_recursion(?Send)]
async fn show_response<'a>(
    response: &'a ResponseMessage,
    client: ManyClient<impl Identity + 'a>,
    r#async: bool,
) -> Result<(), anyhow::Error> {
    let ResponseMessage {
        data, attributes, ..
    } = response;

    let payload = data.clone()?;
    if payload.is_empty() {
        let attr = attributes.get::<AsyncAttribute>().unwrap();
        info!("Async token: {}", hex::encode(&attr.token));

        // Allow eprint/ln for showing the progress bar, when we're interactive.
        #[allow(clippy::print_stderr)]
        fn progress(str: &str, done: bool) {
            if atty::is(atty::Stream::Stderr) {
                if done {
                    eprintln!("{str}");
                } else {
                    eprint!("{str}");
                }
            }
        }

        if !r#async {
            progress("Waiting.", false);

            // TODO: improve on this by using duration and thread and watchdog.
            // Wait for the server for ~60 seconds by pinging it every second.
            for _ in 0..60 {
                let response = client
                    .call(
                        "async.status",
                        StatusArgs {
                            token: attr.token.clone(),
                        },
                    )
                    .await?;
                let status: StatusReturn = minicbor::decode(&response.data?)?;
                match status {
                    StatusReturn::Done { response } => {
                        progress(".", true);
                        let response: ResponseMessage =
                            minicbor::decode(&response.payload.ok_or_else(|| {
                                anyhow!("Envelope with empty payload. Expected ResponseMessage")
                            })?)?;
                        return show_response(&response, client, r#async).await;
                    }
                    StatusReturn::Expired => {
                        progress(".", true);
                        info!("Async token expired before we could check it.");
                        return Ok(());
                    }
                    _ => {
                        progress(".", false);
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            }
        }
    } else {
        println!(
            "{}",
            cbor_diag::parse_bytes(&payload).unwrap().to_diag_pretty()
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn message(
    s: Url,
    to: Address,
    key: impl Identity,
    method: String,
    data: Vec<u8>,
    timestamp: Timestamp,
    r#async: bool,
    delegation: Option<(Address, DelegationAttribute)>,
) -> Result<(), anyhow::Error> {
    let address = if let Some(d) = &delegation {
        d.0
    } else {
        key.address()
    };
    let client = ManyClient::new(s, to, key).unwrap();

    let mut nonce = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce);

    let mut builder = RequestMessageBuilder::default();
    builder
        .version(1)
        .from(address)
        .to(to)
        .method(method)
        .timestamp(timestamp)
        .data(data)
        .nonce(nonce.to_vec());

    let message: RequestMessage = builder
        .build()
        .map_err(|_| ManyError::internal_server_error())?;

    let message = if let Some(d) = delegation {
        message.with_attribute(d.1.try_into().unwrap())
    } else {
        message
    };

    let response = client.send_message(message).await?;

    show_response(&response, client, r#async).await
}

async fn message_from_hex(
    s: Url,
    to: Address,
    key: impl Identity,
    hex: String,
    r#async: bool,
) -> Result<(), anyhow::Error> {
    let client = ManyClient::new(s.clone(), to, key).unwrap();

    let data = hex::decode(hex)?;
    let envelope = CoseSign1::from_slice(&data).map_err(|e| anyhow!(e))?;

    let cose_sign1 = many_client::client::send_envelope(s, envelope).await?;
    let response = ResponseMessage::decode_and_verify(
        &cose_sign1,
        &(AnonymousVerifier, CoseKeyVerifier),
        &BaseIdentityResolver,
    )
    .map_err(|e| anyhow!(e))?;

    show_response(&response, client, r#async).await
}

async fn create_webauthn_identity(
    rp: ManyUrl,
    origin: Option<ManyUrl>,
    phrase: Option<String>,
    address: Option<Address>,
    rp_id: Option<String>,
) -> WebAuthnIdentity {
    let client = ManyClient::new(rp.clone(), Address::anonymous(), AnonymousIdentity)
        .expect("Could not create client");

    let response = if let Some(phrase) = phrase {
        client
            .call(
                "idstore.getFromRecallPhrase",
                idstore::GetFromRecallPhraseArgs(phrase.split(' ').map(String::from).collect()),
            )
            .await
            .unwrap()
    } else if let Some(address) = address {
        client
            .call(
                "idstore.getFromAddress",
                idstore::GetFromAddressArgs(address),
            )
            .await
            .unwrap()
    } else {
        error!("Must specify a phrase or address.");
        process::exit(3);
    };

    let get_returns = response.data.expect("Error from the server");
    let get_returns =
        minicbor::decode::<idstore::GetReturns>(&get_returns).expect("Deserialization error");

    let origin = origin.unwrap_or(rp);

    WebAuthnIdentity::authenticate(
        origin.clone(),
        rp_id.unwrap_or(origin.host_str().expect("Origin has no host").to_string()),
        get_returns,
    )
    .expect("Could not create Identity object")
}

#[tokio::main]
async fn main() {
    let Opts {
        verbosity,
        subcommand,
    } = Opts::parse();
    tracing_subscriber::fmt()
        .with_max_level(verbosity.level())
        .init();

    match subcommand {
        SubCommand::Id(o) => {
            if let Ok(data) = hex::decode(&o.arg) {
                match Address::try_from(data.as_slice()) {
                    Ok(mut i) => {
                        if let Some(subid) = o.subid {
                            i = i
                                .with_subresource_id(subid)
                                .expect("Invalid subresource id");
                        }
                        println!("{i}")
                    }
                    Err(e) => {
                        error!("Identity did not parse: {:?}", e.to_string());
                        process::exit(1);
                    }
                }
            } else if let Ok(mut i) = Address::try_from(o.arg.clone()) {
                if let Some(subid) = o.subid {
                    i = i
                        .with_subresource_id(subid)
                        .expect("Invalid subresource id");
                }
                println!("{}", hex::encode(i.to_vec()));
            } else if let Ok(pem_content) = std::fs::read_to_string(&o.arg) {
                // Create the identity from the public key hash.
                let mut i = CoseKeyIdentity::from_pem(pem_content).unwrap().address();
                if let Some(subid) = o.subid {
                    i = i
                        .with_subresource_id(subid)
                        .expect("Invalid subresource id");
                }

                println!("{i}");
            } else {
                error!("Could not understand the argument.");
                process::exit(2);
            }
        }
        SubCommand::HsmId(o) => {
            let keyid = hex::decode(o.keyid).expect("Failed to decode keyid to hex");

            {
                let mut hsm = Hsm::get_instance().expect("HSM mutex poisoned");
                hsm.init(o.module, keyid)
                    .expect("Failed to initialize HSM module");

                // The session will stay open until the application terminates
                hsm.open_session(o.slot, HsmSessionType::RO, None, None)
                    .expect("Failed to open HSM session");
            }

            let mut id = HsmIdentity::new(HsmMechanismType::ECDSA)
                .expect("Unable to create CoseKeyIdentity from HSM")
                .address();

            if let Some(subid) = o.subid {
                id = id
                    .with_subresource_id(subid)
                    .expect("Invalid subresource id");
            }

            println!("{id}");
        }
        SubCommand::WebauthnId(o) => {
            let identity = create_webauthn_identity(o.rp, None, o.phrase, o.address, None).await;
            println!("{}", identity.address());
        }
        SubCommand::Message(o) => {
            let to_identity = o.to.unwrap_or_default();
            let timestamp = o.timestamp.map_or_else(Timestamp::now, |secs| {
                Timestamp::from_system_time(
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::new(secs, 0))
                        .expect("Invalid timestamp"),
                )
                .expect("Invalid time")
            });
            let data = o
                .data
                .map_or(vec![], |d| cbor_diag::parse_diag(d).unwrap().to_bytes());

            let from_identity: Box<dyn Identity> = if let (Some(module), Some(slot), Some(keyid)) =
                (o.module, o.slot, o.keyid)
            {
                trace!("Getting user PIN");
                let pin = rpassword::prompt_password("Please enter the HSM user PIN: ")
                    .expect("I/O error when reading HSM PIN");
                let keyid = hex::decode(keyid).expect("Failed to decode keyid to hex");

                {
                    let mut hsm = Hsm::get_instance().expect("HSM mutex poisoned");
                    hsm.init(module, keyid)
                        .expect("Failed to initialize HSM module");

                    // The session will stay open until the application terminates
                    hsm.open_session(slot, HsmSessionType::RO, Some(HsmUserType::User), Some(pin))
                        .expect("Failed to open HSM session");
                }

                // Only ECDSA is supported at the moment. It should be easy to add support for
                // new EC mechanisms.
                Box::new(
                    HsmIdentity::new(HsmMechanismType::ECDSA)
                        .expect("Unable to create CoseKeyIdentity from HSM"),
                )
            } else if let Some(p) = o.pem {
                // If `pem` is not provided, use anonymous and don't sign.
                Box::new(CoseKeyIdentity::from_pem(std::fs::read_to_string(p).unwrap()).unwrap())
            } else if o.webauthn {
                let rp =
                    o.rp.as_ref()
                        .or(o.server.as_ref())
                        .expect("Must pass a server or --rp");
                let identity = create_webauthn_identity(
                    rp.clone(),
                    o.webauthn_origin,
                    o.phrase,
                    o.address,
                    o.rp_id,
                )
                .await;
                Box::new(identity)
            } else {
                Box::new(AnonymousIdentity)
            };

            let delegation = o.delegation.map(|values| {
                let certificates = values
                    .iter()
                    .flat_map(|p| pem::parse_many(std::fs::read_to_string(p).unwrap()).unwrap())
                    .filter(|pem| pem.tag == "MANY DELEGATION CERTIFICATE")
                    .map(|pem| CoseSign1::from_slice(&pem.contents).unwrap())
                    .collect::<Vec<CoseSign1>>();

                // Create the attribute.
                let attr = DelegationAttribute::new(certificates);

                // Verify that the identity can delegate to the new identity.
                let resolved_id = attr
                    .resolve(
                        from_identity.address(),
                        &(AnonymousVerifier, CoseKeyVerifier),
                        timestamp,
                    )
                    .unwrap();
                debug!(
                    "Resolved Identity from delegation: {}",
                    resolved_id.to_string()
                );

                (resolved_id, attr)
            });

            if let Some(s) = o.server {
                let result = if let Some(hex) = o.from_hex {
                    message_from_hex(s, to_identity, from_identity, hex, o.r#async).await
                } else {
                    message(
                        s,
                        to_identity,
                        from_identity,
                        o.method.expect("--method is required"),
                        data,
                        timestamp,
                        o.r#async,
                        delegation,
                    )
                    .await
                };

                match result {
                    Ok(()) => {}
                    Err(err) => {
                        error!(
                            "Error returned by server:\n|  {}\n",
                            err.to_string()
                                .split('\n')
                                .collect::<Vec<&str>>()
                                .join("\n|  ")
                        );
                        process::exit(1);
                    }
                }
            } else {
                let message: RequestMessage = RequestMessageBuilder::default()
                    .version(1)
                    .from(from_identity.address())
                    .to(to_identity)
                    .method(o.method.expect("--method is required"))
                    .data(data)
                    .build()
                    .unwrap();

                let cose = encode_cose_sign1_from_request(message, &from_identity).unwrap();
                let bytes = cose.to_vec().unwrap();
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
            let key = Arc::new(
                CoseKeyIdentity::from_pem(&pem)
                    .expect("Could not generate identity from PEM file."),
            );

            let many = ManyServer::simple(
                o.name,
                Arc::clone(&key),
                (AnonymousVerifier, CoseKeyVerifier),
                Some(env!("CARGO_PKG_VERSION").to_string()),
            );
            if o.delegation {
                struct DelegationImpl;
                impl DelegationModuleBackend for DelegationImpl {}

                let mut many_locked = many.lock().unwrap();
                many_locked.add_module(many_modules::delegation::DelegationModule::new(Arc::new(
                    sync::Mutex::new(DelegationImpl),
                )));
            }

            let mockfile = o.mockfile.unwrap_or_default();
            if !mockfile.is_empty() {
                let mut many_locked = many.lock().unwrap();
                let mock_server = ManyMockServer::new(mockfile, None, key);
                many_locked.set_fallback_module(mock_server);
            }
            HttpServer::new(many).bind(o.addr).await.unwrap();
        }
        SubCommand::GetTokenId(o) => {
            let client = ManyClient::new(o.server, Address::anonymous(), AnonymousIdentity)
                .expect("Could not create a client");
            let status = client.status().await.expect("Cannot get status of server");

            if !status.attributes.contains(&ledger::LEDGER_MODULE_ATTRIBUTE) {
                error!("Server does not implement Ledger Attribute.");
                process::exit(1);
            }

            let info: ledger::InfoReturns = minicbor::decode(
                &client
                    .call("ledger.info", ledger::InfoArgs {})
                    .await
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

            println!("{id}");
        }
        SubCommand::Delegation(o) => {
            delegation::delegation(&o).unwrap();
        }
    }
}
