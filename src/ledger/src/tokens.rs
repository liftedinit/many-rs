use anyhow::anyhow;
use clap::{Args, Parser};
use many_cli_helpers::error::ClientServerError;
use many_client::client::blocking::ManyClient;
use many_identity::{Address, Identity};
use many_modules::ledger::extended_info::visual_logo::VisualTokenLogo;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{
    TokenAddExtendedInfoArgs, TokenAddExtendedInfoReturns, TokenBurnArgs, TokenBurnReturns,
    TokenCreateArgs, TokenCreateReturns, TokenInfoArgs, TokenInfoReturns, TokenMintArgs,
    TokenMintReturns, TokenRemoveExtendedInfoArgs, TokenRemoveExtendedInfoReturns, TokenUpdateArgs,
    TokenUpdateReturns,
};
use many_types::cbor::CborNull;
use many_types::ledger::{LedgerTokensAddressMap, TokenAmount, TokenInfoSummary, TokenMaybeOwner};
use many_types::{AttributeRelatedIndex, Memo};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Parser)]
pub struct CommandOpt {
    #[clap(subcommand)]
    /// Token subcommand to execute.
    subcommand: SubcommandOpt,
}

#[derive(Parser)]
enum SubcommandOpt {
    /// Create a new token
    Create(CreateTokenOpt),

    /// Update an existing token
    Update(UpdateTokenOpt),

    /// Add extended information to token
    AddExtInfo(AddExtInfoOpt),

    /// Remove extended information from token
    RemoveExtInfo(RemoveExtInfoOpt),

    /// Get token info
    Info(InfoOpt),

    /// Mint new tokens
    Mint(MintOpt),

    /// Burn tokens
    Burn(BurnOpt),
}

#[derive(Args)]
struct MintOpt {
    symbol: String,

    #[clap(parse(try_from_str = serde_json::from_str))]
    distribution: LedgerTokensAddressMap,

    #[clap(long, parse(try_from_str = Memo::try_from))]
    memo: Option<Memo>,
}

#[derive(Args)]
struct BurnOpt {
    symbol: String,

    #[clap(parse(try_from_str = serde_json::from_str))]
    distribution: LedgerTokensAddressMap,

    #[clap(long, parse(try_from_str = Memo::try_from))]
    memo: Option<Memo>,

    #[clap(long, action)]
    error_on_under_burn: bool,
}

#[derive(Args)]
struct InfoOpt {
    symbol: Address,

    #[clap(long)]
    #[clap(value_parser = attribute_related_index)]
    indices: Option<Vec<AttributeRelatedIndex>>,
}

#[derive(Args)]
struct InitialDistribution {
    #[clap(long)]
    id: Address,

    #[clap(long)]
    amount: u64,
}

#[derive(Parser)]
struct CreateTokenOpt {
    name: String,
    ticker: String,
    decimals: u64,

    #[clap(long)]
    #[clap(value_parser = token_maybe_owner)]
    owner: Option<TokenMaybeOwner>,

    #[clap(long, parse(try_from_str = serde_json::from_str))]
    initial_distribution: Option<LedgerTokensAddressMap>,

    #[clap(long)]
    maximum_supply: Option<u64>,

    #[clap(subcommand)]
    extended_info: Option<CreateExtInfoOpt>,

    #[clap(long)]
    #[clap(parse(try_from_str = Memo::try_from))]
    memo: Option<Memo>,
}

#[derive(Parser)]
struct UpdateTokenOpt {
    symbol: Address,

    #[clap(long)]
    name: Option<String>,

    #[clap(long)]
    ticker: Option<String>,

    #[clap(long)]
    decimals: Option<u64>,

    #[clap(long)]
    #[clap(value_parser = token_maybe_owner)]
    owner: Option<TokenMaybeOwner>,

    #[clap(long)]
    #[clap(parse(try_from_str = Memo::try_from))]
    memo: Option<Memo>,
}

#[derive(Parser)]
enum CreateExtInfoOpt {
    Memo(MemoOpt),
    Logo(LogoOpt),
}

#[derive(Parser)]
struct MemoOpt {
    #[clap(parse(try_from_str = Memo::try_from))]
    memo: Memo,
}

#[derive(Parser)]
struct LogoOpt {
    #[clap(subcommand)]
    logo_type: CreateLogoOpt,
}

#[derive(Parser)]
enum CreateLogoOpt {
    Unicode(UnicodeLogoOpt),
    Image(ImageLogoOpt),
}

#[derive(Parser)]
struct UnicodeLogoOpt {
    glyph: char,
}

#[derive(Parser)]
struct ImageLogoOpt {
    image: PathBuf,
}

#[derive(Parser)]
struct AddExtInfoOpt {
    symbol: Address,

    #[clap(subcommand)]
    ext_info_type: CreateExtInfoOpt,

    #[clap(long)]
    #[clap(parse(try_from_str = Memo::try_from))]
    memo: Option<Memo>,
}

#[derive(Parser)]
struct RemoveExtInfoOpt {
    symbol: Address,

    #[clap(value_parser = attribute_related_index)]
    indices: Vec<AttributeRelatedIndex>,

    #[clap(long)]
    #[clap(parse(try_from_str = Memo::try_from))]
    memo: Option<Memo>,
}

/// Create `TokenMaybeOwner` from CLI `str`
fn token_maybe_owner(s: &str) -> Result<TokenMaybeOwner, String> {
    match s {
        "null" => Ok(TokenMaybeOwner::Right(CborNull)),
        _ => Ok(TokenMaybeOwner::Left(
            Address::try_from(s.to_string()).map_err(|e| e.to_string())?,
        )),
    }
}

fn attribute_related_index(s: &str) -> Result<AttributeRelatedIndex, String> {
    Ok(AttributeRelatedIndex::new(
        s.parse::<u32>().map_err(|e| e.to_string())?,
    ))
}

fn create_ext_info(opts: CreateExtInfoOpt) -> TokenExtendedInfo {
    match opts {
        CreateExtInfoOpt::Memo(opts) => TokenExtendedInfo::new().with_memo(opts.memo).unwrap(),
        CreateExtInfoOpt::Logo(opts) => {
            let mut logo = VisualTokenLogo::new();
            match opts.logo_type {
                CreateLogoOpt::Unicode(opts) => {
                    logo.unicode_front(opts.glyph);
                }
                CreateLogoOpt::Image(opts) => {
                    let content_type = mime_guess::from_path(&opts.image)
                        .first_raw()
                        .expect("Unable to guess the MIME type of image");
                    let binary = std::fs::read(opts.image).expect("Unable to read image");
                    logo.image_front(content_type, binary);
                }
            }
            TokenExtendedInfo::new().with_visual_logo(logo).unwrap()
        }
    }
}

fn create_token(
    client: ManyClient<impl Identity>,
    opts: CreateTokenOpt,
) -> Result<(), ClientServerError> {
    let extended_info = opts.extended_info.map(create_ext_info);

    let args = TokenCreateArgs {
        summary: TokenInfoSummary {
            name: opts.name,
            ticker: opts.ticker,
            decimals: opts.decimals,
        },
        owner: opts.owner,
        initial_distribution: opts.initial_distribution,
        maximum_supply: opts.maximum_supply.map(TokenAmount::from),
        extended_info,
        memo: opts.memo,
    };
    let response = client.call("tokens.create", args)?;
    let payload = crate::wait_response(client, response)?;
    let result: TokenCreateReturns = minicbor::decode(&payload)?;

    println!("{result:#?}");
    Ok(())
}

fn update_token(
    client: ManyClient<impl Identity>,
    opts: UpdateTokenOpt,
) -> Result<(), ClientServerError> {
    let args = TokenUpdateArgs {
        symbol: opts.symbol,
        name: opts.name,
        ticker: opts.ticker,
        decimals: opts.decimals,
        owner: opts.owner,
        memo: opts.memo,
    };
    let response = client.call("tokens.update", args)?;
    let payload = crate::wait_response(client, response)?;
    let _result: TokenUpdateReturns = minicbor::decode(&payload)?;

    Ok(())
}

fn add_ext_info(
    client: ManyClient<impl Identity>,
    opts: AddExtInfoOpt,
) -> Result<(), ClientServerError> {
    let extended_info = create_ext_info(opts.ext_info_type);

    let args = TokenAddExtendedInfoArgs {
        symbol: opts.symbol,
        extended_info,
        memo: opts.memo,
    };
    let response = client.call("tokens.addExtendedInfo", args)?;
    let payload = crate::wait_response(client, response)?;
    let _result: TokenAddExtendedInfoReturns = minicbor::decode(&payload)?;
    Ok(())
}

fn remove_ext_info(
    client: ManyClient<impl Identity>,
    opts: RemoveExtInfoOpt,
) -> Result<(), ClientServerError> {
    let args = TokenRemoveExtendedInfoArgs {
        symbol: opts.symbol,
        extended_info: opts.indices,
        memo: opts.memo,
    };
    let response = client.call("tokens.removeExtendedInfo", args)?;
    let payload = crate::wait_response(client, response)?;
    let _result: TokenRemoveExtendedInfoReturns = minicbor::decode(&payload)?;
    Ok(())
}

fn info_token(client: ManyClient<impl Identity>, opts: InfoOpt) -> Result<(), ClientServerError> {
    let args = TokenInfoArgs {
        symbol: opts.symbol,
        extended_info: opts.indices,
    };
    let response = client.call("tokens.info", args)?;
    let payload = crate::wait_response(client, response)?;
    let result: TokenInfoReturns = minicbor::decode(&payload)?;

    println!("{result:#?}");
    Ok(())
}

fn mint_token(client: ManyClient<impl Identity>, opts: MintOpt) -> Result<(), ClientServerError> {
    let symbol = Address::try_from(opts.symbol.as_str()).or_else(|_| {
        // Get symbol address from name
        let info: many_modules::ledger::InfoReturns =
            minicbor::decode(&client.call_("ledger.info", ())?).unwrap();
        let local_names: BTreeMap<String, Address> = info
            .local_names
            .iter()
            .map(|(x, y)| (y.clone(), *x))
            .collect();
        local_names
            .get(&opts.symbol)
            .cloned()
            .ok_or_else(|| anyhow!("Symbol address not found."))
    })?;
    let args = TokenMintArgs {
        symbol,
        distribution: opts.distribution,
        memo: opts.memo,
    };
    let response = client.call("tokens.mint", args)?;
    let payload = crate::wait_response(client, response)?;
    let result: TokenMintReturns = minicbor::decode(&payload)?;

    println!("{result:#?}");
    Ok(())
}

fn burn_token(client: ManyClient<impl Identity>, opts: BurnOpt) -> Result<(), ClientServerError> {
    let symbol = Address::try_from(opts.symbol.as_str()).or_else(|_| {
        // Get symbol address from name
        let info: many_modules::ledger::InfoReturns =
            minicbor::decode(&client.call_("ledger.info", ())?).unwrap();
        let local_names: BTreeMap<String, Address> = info
            .local_names
            .iter()
            .map(|(x, y)| (y.clone(), *x))
            .collect();
        local_names
            .get(&opts.symbol)
            .cloned()
            .ok_or_else(|| anyhow!("Symbol address not found."))
    })?;
    let args = TokenBurnArgs {
        symbol,
        distribution: opts.distribution,
        memo: opts.memo,
        error_on_under_burn: Some(opts.error_on_under_burn),
    };
    let response = client.call("tokens.burn", args)?;
    let payload = crate::wait_response(client, response)?;
    let result: TokenBurnReturns = minicbor::decode(&payload)?;

    println!("{result:#?}");
    Ok(())
}

pub fn tokens(
    client: ManyClient<impl Identity>,
    opts: CommandOpt,
) -> Result<(), ClientServerError> {
    match opts.subcommand {
        SubcommandOpt::Create(opts) => create_token(client, opts),
        SubcommandOpt::Update(opts) => update_token(client, opts),
        SubcommandOpt::AddExtInfo(opts) => add_ext_info(client, opts),
        SubcommandOpt::RemoveExtInfo(opts) => remove_ext_info(client, opts),
        SubcommandOpt::Info(opts) => info_token(client, opts),
        SubcommandOpt::Mint(opts) => mint_token(client, opts),
        SubcommandOpt::Burn(opts) => burn_token(client, opts),
    }
}
