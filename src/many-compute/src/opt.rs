use clap::Parser;

#[derive(Debug, Parser)]
pub struct AkashOpt {
    pub akash_wallet: String,

    #[clap(long, default_value = "akashnet-2")]
    pub akash_chain_id: String,

    // Akash needs the port number in the url even if the schema is known.
    // Unfortunately, the `url` crate drops the port number from the serialization when the schema is known.
    // TODO: Make `ManyUrl` a real wrapper with a `to_string_with_port` method.
    #[clap(long, default_value = "https://rpc.akashnet.net:443")]
    pub akash_rpc: String,

    #[clap(long, default_value = "auto")]
    pub akash_gas: String,

    #[clap(long, default_value = "1.25")]
    pub akash_gas_adjustment: f64,

    #[clap(long, default_value = "0.025uakt")]
    pub akash_gas_price: String,

    #[clap(long, default_value = "amino-json")]
    pub akash_sign_mode: String,

    #[clap(long, default_value = "os")]
    pub akash_keyring_backend: String,
}
