use crate::storage::ComputeStorage;
use crate::{error};
use many_error::ManyError;
use many_identity::Address;
use many_modules::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, BeginBlockReturn, EndpointInfo, InitChainReturn,
    ManyAbciModuleBackend,
};
use many_modules::compute::{
    CloseArgs, CloseReturns, ComputeModuleBackend, DeployArgs, DeployReturns, InfoArg, InfoReturns,
    ListArgs, ListReturns,
};
use many_types::compute::{
    Bids, ComputeListFilter, ComputeStatus, DeploymentInfo, DeploymentMeta, LeaseStatus,
    LeasesResponse, ProviderInfo, ServiceProtocol, ServiceStatus, TxLog,
};
use many_types::Timestamp;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};
use std::thread::sleep;
use std::time::Duration;
use tracing::{debug, info};
use crate::opt::AkashOpt;

pub mod allow_addrs;

const AKASH_BIN: &str = "provider-services";
const DEPLOYMENT_TIMEOUT: u16 = 60 * 2; // 2 minutes

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

    fn execute_akash_command(&self, args: &[&str]) -> Result<Output, ManyError> {
        Command::new(AKASH_BIN)
            .args(args)
            .output()
            .map_err(|_| ManyError::unknown("Failed to execute command"))
    }

    fn generate_cert(&mut self) -> Result<(), ManyError> {
        // Generate certificate
        info!("Generating certificate");
        let cert_generate_args = [
            "tx",
            "cert",
            "generate",
            "client",
            "--chain-id",
            self.akash_opt.akash_chain_id.as_str(),
            "--node",
            self.akash_opt.akash_rpc.as_str(),
            "--from",
            self.akash_opt.akash_wallet.as_str(),
            "--keyring-backend",
            self.akash_opt.akash_keyring_backend.as_str(),
            "--yes",
        ];
        let output = self.execute_akash_command(&cert_generate_args)?;

        // Certificate exists, continue with deployment
        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
            if err != "Error: certificate error: cannot overwrite certificate\n" {
                return Err(ManyError::unknown("Failed to generate client certificate"));
            }
            info!("Certificate already exists, continuing");
        } else {
            info!("Publishing certificate");
            let cert_publish_args = [
                "tx",
                "cert",
                "publish",
                "client",
                "--chain-id",
                self.akash_opt.akash_chain_id.as_str(),
                "--node",
                self.akash_opt.akash_rpc.as_str(),
                "--from",
                self.akash_opt.akash_wallet.as_str(),
                "--keyring-backend",
                self.akash_opt.akash_keyring_backend.as_str(),
                "--yes",
            ];
            let output = self.execute_akash_command(&cert_publish_args)?;

            if !output.status.success() {
                let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
                return Err(ManyError::unknown(format!(
                    "Failed to publish client certificate: {}",
                    err
                )));
            }
        }
        Ok(())
    }

    fn create_deployment(
        &mut self,
        args: &DeployArgs,
    ) -> Result<(u64, u64, u64, String), ManyError> {
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
            image, port, num_cpu, num_memory, memory_type, num_storage, storage_type, region
        );

        debug!("{sdl}");

        let mut tmpfile = tempfile::Builder::new()
            .prefix("akash-sdl")
            .suffix(".yml")
            .tempfile()
            .map_err(ManyError::unknown)?;
        write!(tmpfile, "{}", sdl).map_err(ManyError::unknown)?;
        let tmpfile_path = tmpfile
            .path()
            .to_str()
            .ok_or(ManyError::unknown("Unable to get SDL file path"))?;

        info!("Creating deployment");
        let deploy_args = [
            "tx",
            "deployment",
            "create",
            tmpfile_path,
            "--chain-id",
            self.akash_opt.akash_chain_id.as_str(),
            "--node",
            self.akash_opt.akash_rpc.as_str(),
            "--gas",
            self.akash_opt.akash_gas.as_str(),
            "--gas-prices",
            self.akash_opt.akash_gas_price.as_str(),
            "--gas-adjustment",
            &format!("{}", self.akash_opt.akash_gas_adjustment),
            "--sign-mode",
            self.akash_opt.akash_sign_mode.as_str(),
            "--from",
            self.akash_opt.akash_wallet.as_str(),
            "--keyring-backend",
            self.akash_opt.akash_keyring_backend.as_str(),
            "--yes",
        ];
        let output = self.execute_akash_command(&deploy_args)?;

        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
            return Err(ManyError::unknown(format!(
                "akash tx deployment create failed: {err}"
            )));
        }

        let response: TxLog = serde_json::from_slice(&output.stdout).map_err(ManyError::unknown)?;

        let mut seq_values: HashMap<String, u64> = HashMap::new();
        let keys = vec!["dseq", "gseq", "oseq"];

        for log in response.logs {
            for event in log.events {
                for attr in event.attributes {
                    if keys.contains(&attr.key.as_str()) {
                        let value = attr.value.parse().map_err(ManyError::unknown)?;
                        seq_values.insert(attr.key, value);
                    }
                }
            }
        }

        let dseq = seq_values.get("dseq").unwrap_or(&0);
        let gseq = seq_values.get("gseq").unwrap_or(&0);
        let oseq = seq_values.get("oseq").unwrap_or(&0);

        debug!("dseq: {dseq}, gseq: {gseq}, oseq: {oseq}");
        Ok((*dseq, *gseq, *oseq, sdl))
    }

    // TODO: Handle price range
    fn create_bid(&mut self, dseq: u64, gseq: u64, oseq: u64) -> Result<(String, f64), ManyError> {
        let mut my_bids = vec![];
        let mut counter = 0;

        while my_bids.is_empty() && counter < DEPLOYMENT_TIMEOUT {
            info!("Waiting for bid to be created");
            let bid_list_args = [
                "query",
                "market",
                "bid",
                "list",
                "--chain-id",
                self.akash_opt.akash_chain_id.as_str(),
                "--node",
                self.akash_opt.akash_rpc.as_str(),
                "--owner",
                self.akash_opt.akash_wallet.as_str(),
                "--dseq",
                &dseq.to_string(),
                "--gseq",
                &gseq.to_string(),
                "--oseq",
                &oseq.to_string(),
                "--state",
                "open",
            ];
            let output = self.execute_akash_command(&bid_list_args)?;

            if !output.status.success() {
                let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
                return Err(ManyError::unknown(format!(
                    "akash query market bid list failed: {err}"
                )));
            }

            let response: Bids =
                serde_yaml::from_slice(&output.stdout).map_err(ManyError::unknown)?;
            my_bids = response.bids;

            sleep(Duration::from_secs(1));
            counter += 1;
        }

        let mut cheapest_provider = "".to_string();
        let mut cheapest_price = f64::MAX;

        dbg!(&my_bids);

        // Find the cheapest bid
        for bid in my_bids {
            if bid.bid.price.amount.partial_cmp(&cheapest_price) == Some(Ordering::Less) {
                cheapest_price = bid.bid.price.amount;
                cheapest_provider = bid.bid.bid_id.provider;
            }
        }

        debug!("cheapest_provider: {cheapest_provider}");
        debug!("cheapest_price: {cheapest_price}");

        // TODO: Handle price range
        Ok((cheapest_provider, cheapest_price))
    }

    fn create_lease(
        &mut self,
        dseq: u64,
        gseq: u64,
        oseq: u64,
        provider: &String,
    ) -> Result<(), ManyError> {
        info!("Creating lease");
        let lease_create_args = [
            "tx",
            "market",
            "lease",
            "create",
            "--chain-id",
            self.akash_opt.akash_chain_id.as_str(),
            "--node",
            self.akash_opt.akash_rpc.as_str(),
            "--gas",
            self.akash_opt.akash_gas.as_str(),
            "--gas-prices",
            self.akash_opt.akash_gas_price.as_str(),
            "--gas-adjustment",
            &self.akash_opt.akash_gas_adjustment.to_string(),
            "--sign-mode",
            self.akash_opt.akash_sign_mode.as_str(),
            "--keyring-backend",
            self.akash_opt.akash_keyring_backend.as_str(),
            "--from",
            self.akash_opt.akash_wallet.as_str(),
            "--dseq",
            &dseq.to_string(),
            "--gseq",
            &gseq.to_string(),
            "--oseq",
            &oseq.to_string(),
            "--provider",
            provider,
            "--yes",
        ];
        let output = self.execute_akash_command(&lease_create_args)?;

        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;

            // An error occurred while creating the lease, close the deployment
            self.close_deployment(&CloseArgs { dseq })?;

            return Err(ManyError::unknown(format!(
                "akash tx market lease create failed: {err}"
            )));
        }

        Ok(())
    }

    fn check_lease_status(&mut self, dseq: u64, gseq: u64, oseq: u64) -> Result<(), ManyError> {
        info!("Checking lease status");
        let mut counter = 0;
        while counter < DEPLOYMENT_TIMEOUT {
            let output = Command::new(AKASH_BIN)
                .args(["query", "market", "lease", "list"])
                .args(["--chain-id", self.akash_opt.akash_chain_id.as_str()])
                .args(["--node", self.akash_opt.akash_rpc.as_str()])
                .args(["--owner", self.akash_opt.akash_wallet.as_str()])
                .args(["--dseq", &dseq.to_string()])
                .args(["--gseq", &gseq.to_string()])
                .args(["--oseq", &oseq.to_string()])
                .output()
                .map_err(ManyError::unknown)?;

            if !output.status.success() {
                let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;

                // An error occurred while creating the lease, close the deployment
                self.close_deployment(&CloseArgs { dseq })?;

                return Err(ManyError::unknown(format!(
                    "akash query market lease list failed: {err}"
                )));
            }

            let response: LeasesResponse =
                serde_yaml::from_slice(&output.stdout).map_err(ManyError::unknown)?;
            if !response.leases.is_empty() {
                for lease in response.leases {
                    if lease.lease.state == "active" {
                        return Ok(());
                    }
                }

                // An error occurred while creating the lease, close the deployment
                self.close_deployment(&CloseArgs { dseq })?;
                return Err(ManyError::unknown("active lease not found"));
            }

            sleep(Duration::from_secs(1));
            counter += 1;
        }

        // An error occurred while creating the lease, close the deployment
        self.close_deployment(&CloseArgs { dseq })?;
        Err(ManyError::unknown("active lease not found"))
    }

    fn check_manifest_status(
        &mut self,
        dseq: u64,
        gseq: u64,
        oseq: u64,
        provider: &String,
    ) -> Result<LeaseStatus, ManyError> {
        info!("Checking manifest status");
        let mut counter = 0;
        while counter < DEPLOYMENT_TIMEOUT {
            let lease_list_args = [
                "lease-status",
                "--node",
                self.akash_opt.akash_rpc.as_str(),
                "--from",
                self.akash_opt.akash_wallet.as_str(),
                "--dseq",
                &dseq.to_string(),
                "--gseq",
                &gseq.to_string(),
                "--oseq",
                &oseq.to_string(),
                "--provider",
                provider,
                "--keyring-backend",
                self.akash_opt.akash_keyring_backend.as_str(),
            ];
            let output = self.execute_akash_command(&lease_list_args)?;

            if !output.status.success() {
                let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;

                // An error occurred while creating the lease, close the deployment
                self.close_deployment(&CloseArgs { dseq })?;

                return Err(ManyError::unknown(format!(
                    "akash query market lease list failed: {err}"
                )));
            }

            let response: LeaseStatus =
                serde_yaml::from_slice(&output.stdout).map_err(ManyError::unknown)?;
            if response
                .services
                .get("app")
                .and_then(|service_status| service_status.as_ref())
                .map_or(false, |box ServiceStatus { available, .. }| *available > 0)
            {
                return Ok(response);
            }

            sleep(Duration::from_secs(1));
            counter += 1;
        }

        // An error occurred while creating the lease, close the deployment
        self.close_deployment(&CloseArgs { dseq })?;
        Err(ManyError::unknown("active lease not found"))
    }

    fn close_deployment(&mut self, args: &CloseArgs) -> Result<(), ManyError> {
        info!("Closing deployment");
        let deployment_close_args = [
            "tx",
            "deployment",
            "close",
            "--chain-id",
            self.akash_opt.akash_chain_id.as_str(),
            "--node",
            self.akash_opt.akash_rpc.as_str(),
            "--gas",
            self.akash_opt.akash_gas.as_str(),
            "--gas-prices",
            self.akash_opt.akash_gas_price.as_str(),
            "--gas-adjustment",
            &format!("{}", self.akash_opt.akash_gas_adjustment),
            "--sign-mode",
            self.akash_opt.akash_sign_mode.as_str(),
            "--from",
            self.akash_opt.akash_wallet.as_str(),
            "--dseq",
            &args.dseq.to_string(),
            "--keyring-backend",
            self.akash_opt.akash_keyring_backend.as_str(),
            "--yes",
        ];
        let output = self.execute_akash_command(&deployment_close_args)?;

        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;
            return Err(ManyError::unknown(format!(
                "akash tx deployment close failed: {err}"
            )));
        }

        Ok(())
    }

    fn send_manifest(
        &mut self,
        dseq: u64,
        gseq: u64,
        oseq: u64,
        provider: &String,
        sdl: &String,
    ) -> Result<(), ManyError> {
        info!("Sending manifest");
        let mut tmpfile = tempfile::Builder::new()
            .prefix("akash-sdl")
            .suffix(".yml")
            .tempfile()
            .map_err(ManyError::unknown)?;
        write!(tmpfile, "{}", sdl).map_err(ManyError::unknown)?;
        let tmpfile_path = tmpfile
            .path()
            .to_str()
            .ok_or(ManyError::unknown("Unable to get SDL file path"))?;

        let send_manifest_args = [
            "send-manifest",
            tmpfile_path,
            "--node",
            self.akash_opt.akash_rpc.as_str(),
            "--from",
            self.akash_opt.akash_wallet.as_str(),
            "--dseq",
            &dseq.to_string(),
            "--gseq",
            &gseq.to_string(),
            "--oseq",
            &oseq.to_string(),
            "--provider",
            provider,
            "--keyring-backend",
            self.akash_opt.akash_keyring_backend.as_str(),
        ];
        let output = self.execute_akash_command(&send_manifest_args)?;

        if !output.status.success() {
            let err = std::str::from_utf8(&output.stderr).map_err(ManyError::unknown)?;

            // An error occurred while creating the lease, close the deployment
            self.close_deployment(&CloseArgs { dseq })?;

            return Err(ManyError::unknown(format!(
                "akash send-manifest failed: {err}"
            )));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn create_deployment_meta(
        &self,
        host: Option<String>,
        port: u16,
        external_port: u16,
        protocol: ServiceProtocol,
        dseq: u64,
        provider: String,
        price: f64,
        image: String,
    ) -> DeploymentMeta {
        DeploymentMeta {
            status: ComputeStatus::Deployed,
            dseq,
            meta: Some(DeploymentInfo {
                provider,
                provider_info: ProviderInfo {
                    host,
                    port,
                    external_port,
                    protocol,
                },
                price,
            }),
            image,
        }
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
                ("compute.close".to_string(), EndpointInfo { is_command: true }),
                ("compute.list".to_string(), EndpointInfo { is_command: false }),
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

    fn deploy(&mut self, sender: &Address, args: DeployArgs) -> Result<DeployReturns, ManyError> {
        // At this point, the sender should already be validated by the WhitelistValidator
        self.generate_cert()?;
        let (dseq, gseq, oseq, sdl) = self.create_deployment(&args)?;
        let (provider, price) = self.create_bid(dseq, gseq, oseq)?;

        let DeployArgs { image, port, .. } = args;

        self.create_lease(dseq, gseq, oseq, &provider)?;
        self.check_lease_status(dseq, gseq, oseq)?;
        self.send_manifest(dseq, gseq, oseq, &provider, &sdl)?;
        let lease_status = self.check_manifest_status(dseq, gseq, oseq, &provider)?;

        let uris = lease_status
            .services
            .get("app")
            .and_then(|service_status| service_status.as_ref())
            .and_then(|boxed_status| boxed_status.uris.as_deref());

        let forwarded_ports = lease_status.forwarded_ports.get("app");

        let meta = match (
            uris.and_then(|u| u.get(0)),
            forwarded_ports.and_then(|fp| fp.get(0)),
        ) {
            (Some(uri), _) => self.create_deployment_meta(
                Some(uri.clone()),
                port,
                port,
                ServiceProtocol::TCP,
                dseq,
                provider,
                price,
                image,
            ),
            (_, Some(forwarded_port)) => self.create_deployment_meta(
                forwarded_port.host.clone(),
                port,
                forwarded_port.external_port,
                forwarded_port.proto,
                dseq,
                provider,
                price,
                image,
            ),
            _ => {
                return Err(ManyError::unknown(format!(
                    "No URIs or forwarded ports found for deployment {}",
                    dseq
                )))
            }
        };

        // Write info to compute storage
        self.storage.add_deployment(sender, &meta)?;

        Ok(DeployReturns(meta))
    }

    fn close(&mut self, sender: &Address, args: CloseArgs) -> Result<CloseReturns, ManyError> {
        if !self.storage.has(sender, args.dseq)? {
            return Err(ManyError::unknown(format!(
                "deployment {} not found for {}",
                args.dseq, sender
            )));
        }

        self.close_deployment(&args)?;
        self.storage.remove_deployment(sender, args.dseq)?;

        Ok(CloseReturns {})
    }

    fn list(&self, _sender: &Address, args: ListArgs) -> Result<ListReturns, ManyError> {
        let deployments = self.storage.list_deployment(args.order, args.owner)?;
        Ok(ListReturns {
            deployments: match args.filter {
                Some(ComputeListFilter::Status(filter)) => deployments
                    .into_iter()
                    .filter(|meta| meta.status == filter)
                    .collect(),
                Some(ComputeListFilter::All) | None => deployments,
            },
        })
    }
}
