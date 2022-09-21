use clap::Parser;
use coset::CborSerializable;
use many_error::ManyError;
use many_identity::{Address, Identity};
use many_identity_dsa::CoseKeyIdentity;
use many_types::delegation::Certificate;
use many_types::Timestamp;
use std::ops::Add;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

#[derive(Parser)]
pub struct DelegationOpt {
    #[clap(subcommand)]
    subcommand: DelegationSubCommand,
}

#[derive(Parser)]
enum DelegationSubCommand {
    /// Create a new delegation certificate.
    Create(CreateOpt),
}

#[derive(Parser)]
struct CreateOpt {
    /// The `to` address to delegate to.
    to: Address,

    /// The PEM file to sign with.
    #[clap(short, long)]
    pem: PathBuf,

    /// Expiration (in seconds from time of creation).
    #[clap(short, long)]
    expiration: u64,

    /// Output path.
    output: PathBuf,
}

fn create(opts: &CreateOpt) -> Result<(), ManyError> {
    let expiration =
        Timestamp::from_system_time(SystemTime::now().add(Duration::from_secs(opts.expiration)))
            .unwrap();
    let pem = std::fs::read_to_string(&opts.pem).unwrap();
    let id = CoseKeyIdentity::from_pem(pem)?;

    let cert = Certificate::new(id.address(), opts.to, expiration)
        .sign(&id)
        .unwrap();

    let bytes = cert.to_vec().unwrap();
    let pem_out = pem::Pem {
        tag: "MANY DELEGATION CERTIFICATE".to_string(),
        contents: bytes,
    };

    std::fs::write(&opts.output, pem::encode(&pem_out)).unwrap();

    Ok(())
}

pub fn delegation(opts: &DelegationOpt) -> Result<(), ManyError> {
    match &opts.subcommand {
        DelegationSubCommand::Create(o) => create(o),
    }
}
