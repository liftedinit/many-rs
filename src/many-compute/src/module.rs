use crate::{AkashOpt, error};
use crate::storage::ComputeStorage;
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, BeginBlockReturn, EndpointInfo, InitChainReturn,
    ManyAbciModuleBackend,
};
use many_modules::compute::{CloseArgs, CloseReturns, ComputeModuleBackend, DeployArgs, DeployReturns, InfoArg, InfoReturns};
use many_types::Timestamp;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs::File;
use std::io;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, ExitCode, ExitStatus};
use std::thread::sleep;
use std::time::Duration;
use tracing::{debug, error, info, trace};
use many_types::compute::{ComputeStatus, Protocol, ProviderInfo};

const AKASH_BIN: &str = "provider-services";

const SDL_TEMPLATE: &str = r#"
"#;

// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    identity: Address,
    hash: Option<String>,
}

#[derive(Debug)]
pub struct ComputeModuleImpl {
    akash_opt: AkashOpt,
    storage: ComputeStorage,
}

impl ComputeModuleImpl {
    pub fn load<P: AsRef<Path>>(
        akash_opt: AkashOpt,
        persistent_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage =
            ComputeStorage::load(persistent_store_path, blockchain).map_err(ManyError::unknown)?;

        Ok(Self { akash_opt, storage })
    }

    pub fn new<P: AsRef<Path>>(
        initial_state: InitialStateJson,
        akash_opt: AkashOpt,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage =
            ComputeStorage::new(initial_state.identity, persistence_store_path, blockchain)
                .map_err(ManyError::unknown)?;

        if let Some(h) = initial_state.hash {
            // Verify the hash.
            let actual = hex::encode(storage.hash());
            if actual != h {
                return Err(error::invalid_initial_hash(h, actual));
            }
        }

        info!(
            height = storage.get_height(),
            hash = hex::encode(storage.hash()).as_str()
        );

        Ok(Self { akash_opt, storage })
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl ManyAbciModuleBackend for ComputeModuleImpl {
    #[rustfmt::skip]
    fn init(&mut self) -> Result<AbciInit, ManyError> {
        Ok(AbciInit {
            endpoints: BTreeMap::from([
                ("compute.info".to_string(), EndpointInfo { is_command: false }),
                ("compute.deploy".to_string(), EndpointInfo { is_command: true }),
                //
                // Events
                ("events.info".to_string(), EndpointInfo { is_command: false }),
                ("events.list".to_string(), EndpointInfo { is_command: false }),
            ]),
        })
    }

    fn init_chain(&mut self) -> Result<InitChainReturn, ManyError> {
        info!("abci.init_chain()",);
        Ok(InitChainReturn {})
    }

    fn begin_block(&mut self, info: AbciBlock) -> Result<BeginBlockReturn, ManyError> {
        let time = info.time;
        info!(
            "abci.block_begin(): time={:?} curr_height={}",
            time,
            self.storage.get_height()
        );

        if let Some(time) = time {
            let time = Timestamp::new(time)?;
            self.storage.set_time(time);
        }

        Ok(BeginBlockReturn {})
    }

    fn info(&self) -> Result<AbciInfo, ManyError> {
        let storage = &self.storage;

        info!(
            "abci.info(): height={} hash={}",
            storage.get_height(),
            hex::encode(storage.hash()).as_str()
        );
        Ok(AbciInfo {
            height: storage.get_height(),
            hash: storage.hash().into(),
        })
    }

    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError> {
        let result = self.storage.commit();

        info!(
            "abci.commit(): retain_height={} hash={}",
            result.retain_height,
            hex::encode(result.hash.as_slice()).as_str()
        );
        Ok(result)
    }
}

impl ComputeModuleBackend for ComputeModuleImpl {
    fn info(&self, _sender: &Address, _args: InfoArg) -> Result<InfoReturns, ManyError> {
        // Hash the storage.
        let hash = self.storage.hash();

        Ok(InfoReturns { hash: hash.into() })
    }

    fn deploy(&self, _sender: &Address, args: DeployArgs) -> Result<DeployReturns, ManyError> {
        // At this point, the sender should already be validated by the WhitelistValidator

        let DeployArgs {
            image,
            port,
            num_cpu,
            num_memory,
            memory_type,
            num_storage,
            storage_type,
            region,
        } = args;

        // Generate certificate
        let output = Command::new(AKASH_BIN)
            .args(["tx", "cert", "generate", "client"])
            .args(["--chain-id", self.akash_opt.akash_chain_id.as_str()])
            .args(["--node", self.akash_opt.akash_rpc.as_str()])
            .args(["--from", self.akash_opt.akash_wallet.as_str()])
            .output()
            .map_err(ManyError::unknown)?;

        // Certificate exists, continue with deployment
        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
            if err != "Error: certificate error: cannot overwrite certificate\n" {
                return Err(ManyError::unknown("akash tx cert generate client failed"));
            }
            error!("{err}");
        } else {
            // We don't already have a certificate - publish new akash certificate
            let output = Command::new(AKASH_BIN)
                .args(["tx", "cert", "publish", "client"])
                .args(["--chain-id", self.akash_opt.akash_chain_id.as_str()])
                .args(["--node", self.akash_opt.akash_rpc.as_str()])
                .args(["--from", self.akash_opt.akash_wallet.as_str()])
                .output()
                .map_err(ManyError::unknown)?;

            if !output.status.success() {
                let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
                error!("{err}");
                return Err(ManyError::unknown("akash tx cert publish client failed"));
            }
        }

        // Creating deployment SDL
        let sdl = format!(
r#"---
version: "2.0"

services:
  app:
    image: {}
    expose:
      - port: {}
        to:
          - global: true
profiles:
  compute:
    app:
      resources:
        cpu:
          units: {}
        memory:
          size: {}{}
        storage:
          size: {}{}
  placement:
    region:
      attributes:
        host: akash
        region: {}
      signedBy:
        anyOf:
          - "akash1365yvmc4s7awdyj3n2sav7xfx76adc6dnmlx63"
          - "akash18qa2a2ltfyvkyj0ggj3hkvuj6twzyumuaru9s4"
      pricing:
        app:
          denom: uakt
          amount: 10000
deployment:
  app:
    region:
      profile: app
      count: 1"#,
        image, port, num_cpu, num_memory, memory_type, num_storage, storage_type, region);

        // let mut tmpfile = tempfile::NamedTempFile::new().map_err(ManyError::unknown)?;
        let mut tmpfile = File::create("/tmp/akash-sdl.yml").map_err(ManyError::unknown)?;
        // let mut tmpfile = tempfile::Builder::new()
        //     .prefix("akash-sdl")
        //     .suffix(".yml")
        //     .tempfile()
        //     .map_err(ManyError::unknown)?;
        write!(tmpfile, "{}", sdl).map_err(ManyError::unknown)?;
        // trace!("tmpfile path: {:?}", tmpfile.path());
        debug!("{sdl}");

        // Creating deployment
        let output = Command::new(AKASH_BIN)
            .args(["tx", "deployment", "create", "/tmp/akash-sdl.yml"]) // Safe to unwrap
            .args(["--chain-id", self.akash_opt.akash_chain_id.as_str()])
            .args(["--node", self.akash_opt.akash_rpc.as_str()])
            .args(["--gas", self.akash_opt.akash_gas.as_str()])
            .args(["--gas-prices", self.akash_opt.akash_gas_price.as_str()])
            .args(["--gas-adjustment", &format!("{}", self.akash_opt.akash_gas_adjustment)])
            .args(["--sign-mode", self.akash_opt.akash_sign_mode.as_str()])
            .args(["--from", self.akash_opt.akash_wallet.as_str()])
            .args(["--yes"])
            .output()
            .map_err(ManyError::unknown)?;

        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
            error!("{err}");
            return Err(ManyError::unknown("akash tx deployment create failed"));
        }

        // TODO: Deserialize to proper structs
        let response = serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .map_err(ManyError::unknown)?;

        let attributes = &response["logs"][0]["events"][0]["attributes"];
        if !attributes.is_array() {
            return Err(ManyError::unknown("akash tx deployment create failed. attributes is not an array."));
        }

        let mut dseq = "";
        let mut gseq = "";
        let mut oseq = "";

        // Safe because we checked above
        for attr in attributes.as_array().unwrap() {
            if dseq == "" && attr["key"] == "dseq" {
                dseq = attr["value"].as_str().ok_or(ManyError::unknown("unable to parse dseq."))?;
            }
            if gseq == "" && attr["key"] == "gseq" {
                gseq = attr["value"].as_str().ok_or(ManyError::unknown("unable to parse gseq."))?;
            }
            if oseq == "" && attr["key"] == "oseq" {
                oseq = attr["value"].as_str().ok_or(ManyError::unknown("unable to parse oseq."))?;
            }
        }

        debug!("dseq: {dseq}");
        debug!("gseq: {gseq}");
        debug!("oseq: {oseq}");

        let mut my_bids = vec![];
        let mut counter = 0;

        while my_bids.is_empty() && counter < 10 {
            // View the provider bids
            let output = Command::new(AKASH_BIN)
                .args(["query", "market", "bid", "list"])
                .args(["--chain-id", self.akash_opt.akash_chain_id.as_str()])
                .args(["--node", self.akash_opt.akash_rpc.as_str()])
                .args(["--owner", self.akash_opt.akash_wallet.as_str()])
                .args(["--dseq", dseq])
                .args(["--gseq", gseq])
                .args(["--oseq", oseq])
                .args(["--state", "open"])
                .output()
                .map_err(ManyError::unknown)?;

            if !output.status.success() {
                let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
                error!("{err}");
                return Err(ManyError::unknown("akash query market bid list failed"));
            }

            // TODO: Deserialize to proper structs
            let response = serde_yaml::from_slice::<serde_yaml::Value>(&output.stdout)
                .map_err(ManyError::unknown)?;

            let bids = &response["bids"];
            if !bids.is_sequence() {
                return Err(ManyError::unknown("akash query market bid list failed. bids is not an array."));
            }

            my_bids = bids.as_sequence().unwrap().to_vec();

            sleep(Duration::from_secs(1));
            counter += 1;
        }


        let mut cheapest_provider = "".to_string();
        let mut cheapest_price = f64::MAX;

        // Find the cheapest bid
        for bid in my_bids {
            let price = bid["bid"]["price"]["amount"].as_str().ok_or(ManyError::unknown("unable to parse price."))?;
            let price = price.parse::<f64>().map_err(ManyError::unknown)?;
            if price < cheapest_price {
                cheapest_price = price;
                cheapest_provider = bid["bid"]["bid_id"]["provider"].as_str().ok_or(ManyError::unknown("unable to parse provider."))?.to_string();
            }
        }

        debug!("cheapest_provider: {cheapest_provider}");
        debug!("cheapest_price: {cheapest_price}");

        // TODO: Handle price range

        // Create a lease using the cheapest bid
        let output = Command::new(AKASH_BIN)
            .args(["tx", "market", "lease", "create"])
            .args(["--chain-id", self.akash_opt.akash_chain_id.as_str()])
            .args(["--node", self.akash_opt.akash_rpc.as_str()])
            .args(["--gas", self.akash_opt.akash_gas.as_str()])
            .args(["--gas-prices", self.akash_opt.akash_gas_price.as_str()])
            .args(["--gas-adjustment", &format!("{}", self.akash_opt.akash_gas_adjustment)])
            .args(["--sign-mode", self.akash_opt.akash_sign_mode.as_str()])
            .args(["--from", self.akash_opt.akash_wallet.as_str()])
            .args(["--dseq", dseq])
            .args(["--gseq", gseq])
            .args(["--oseq", oseq])
            .args(["--provider", &cheapest_provider])
            .args(["--yes"])
            .output()
            .map_err(ManyError::unknown)?;

        // TODO: Handle bids timeout

        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
            error!("{err}");
            return Err(ManyError::unknown("akash tx market lease create failed"));
        }

        // Query lease list
        // let output = Command::new(AKASH_BIN)
        //     .args(["query", "market", "lease", "list"])
        //     .args(["--chain-id", self.akash_opt.akash_chain_id.as_str()])
        //     .args(["--node", self.akash_opt.akash_rpc.as_str()])
        //     .args(["--owner", self.akash_opt.akash_wallet.as_str()])
        //     .args(["--dseq", dseq])
        //     .args(["--gseq", gseq])
        //     .args(["--oseq", oseq])
        //     .output()
        //     .map_err(ManyError::unknown)?;

        // FIXME: This is a hack to wait for the lease to be created
        sleep(Duration::from_secs(2));

        // Send the manifest
        let output = Command::new(AKASH_BIN)
            .args(["send-manifest", "/tmp/akash-sdl.yml"])
            .args(["--node", self.akash_opt.akash_rpc.as_str()])
            .args(["--from", self.akash_opt.akash_wallet.as_str()])
            .args(["--dseq", dseq])
            .args(["--gseq", gseq])
            .args(["--oseq", oseq])
            .args(["--provider", &cheapest_provider])
            .output()
            .map_err(ManyError::unknown)?;

        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
            error!("{err}");
            return Err(ManyError::unknown("akash send-manifest failed"));
        }

        // FIXME: This is a hack to wait for the lease to be created
        sleep(Duration::from_secs(2));

        // Get lease status
        let output = Command::new(AKASH_BIN)
            .args(["lease-status"])
            .args(["--node", self.akash_opt.akash_rpc.as_str()])
            .args(["--dseq", dseq])
            .args(["--gseq", gseq])
            .args(["--oseq", oseq])
            .args(["--provider", &cheapest_provider])
            .args(["--from", self.akash_opt.akash_wallet.as_str()])
            .output()
            .map_err(ManyError::unknown)?;

        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
            error!("{err}");
            return Err(ManyError::unknown("akash lease-status failed"));
        }

        // TODO: Deserialize to proper structs
        io::stdout().write_all(&output.stdout).unwrap();

        // Write info to compute storage

        // TODO: Return relevant info...
        Ok(DeployReturns {
            status: ComputeStatus::Running,
            provider_info: ProviderInfo {
                host: "".to_string(),
                port,
                external_port: 0,
                protocol: Protocol::TCP,
            },
            dseq: dseq.parse().map_err(ManyError::unknown)?, // TODO
        })
    }

    fn close(&self, sender: &Address, args: CloseArgs) -> Result<CloseReturns, ManyError> {
        todo!()
    }
}
