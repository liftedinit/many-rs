use crate::TargetCommandOpt;
use clap::Parser;
use many_cli_helpers::error::ClientServerError;
use many_client::client::blocking::ManyClient;
use many_identity::{Address, Identity};
use many_modules::account::features::multisig;
use many_modules::{events, ledger};
use many_protocol::ResponseMessage;
use many_types::ledger::TokenAmount;
use many_types::memo::MemoLegacy;
use many_types::Memo;
use minicbor::bytes::ByteVec;
use tracing::info;

#[derive(Parser)]
pub struct CommandOpt {
    #[clap(subcommand)]
    /// Multisig subcommand to execute.
    subcommand: SubcommandOpt,
}

#[derive(Parser)]
struct SetDefaultsOpt {
    /// The account to set defaults of.
    target_account: Address,

    #[clap(flatten)]
    opts: MultisigArgOpt,
}

#[derive(Parser)]
enum SubcommandOpt {
    /// Submit a new transaction to be approved.
    Submit {
        /// The account to use as the source of the multisig command.
        account: Address,

        /// Memo to use for a transaction.
        #[clap(long)]
        memo: Option<String>,

        /// Legacy memo to use for a transaction.
        #[clap(long)]
        legacy_memo: Option<String>,

        #[clap(flatten)]
        multisig_arg: MultisigArgOpt,

        #[clap(subcommand)]
        subcommand: SubmitOpt,
    },

    /// Approve a transaction.
    Approve(TransactionOpt),

    /// Revoke approval of a transaction.
    Revoke(TransactionOpt),

    /// Execute a transaction.
    Execute(TransactionOpt),

    /// Show the information of a multisig transaction.
    Info(TransactionOpt),

    /// Set new defaults for the multisig account.
    SetDefaults(SetDefaultsOpt),
}

#[derive(Parser)]
enum SubmitOpt {
    /// Send tokens to someone.
    Send(TargetCommandOpt),

    /// Set new defaults for the account.
    SetDefaults(SetDefaultsOpt),
}

fn parse_token(s: &str) -> Result<ByteVec, String> {
    hex::decode(s).map_err(|e| e.to_string()).map(|v| v.into())
}

#[derive(Parser)]
struct TransactionOpt {
    /// The transaction token, obtained when submitting a new transaction.
    #[clap(parse(try_from_str=parse_token))]
    token: ByteVec,
}

#[derive(Parser)]
struct MultisigArgOpt {
    /// The number of approvals needed to execute a transaction.
    #[clap(long)]
    threshold: Option<u64>,

    /// The timeout of a transaction.
    #[clap(long)]
    timeout: Option<humantime::Duration>,

    /// Whether to execute a transaction automatically when the threshold of
    /// approvals is reached.
    #[clap(long)]
    execute_automatically: Option<bool>,
}

fn submit_send(
    client: ManyClient<impl Identity>,
    account: Address,
    multisig_arg: MultisigArgOpt,
    opts: TargetCommandOpt,
    memo: Option<String>,
    legacy_memo: Option<String>,
) -> Result<(), ClientServerError> {
    let TargetCommandOpt {
        account: from,
        identity,
        amount,
        symbol,
        memo: send_memo,
    } = opts;
    let MultisigArgOpt {
        threshold,
        timeout,
        execute_automatically,
    } = multisig_arg;
    let symbol = crate::resolve_symbol(&client, symbol)?;
    let transaction = events::AccountMultisigTransaction::Send(ledger::SendArgs {
        from: from.or(Some(account)),
        to: identity,
        symbol,
        amount: TokenAmount::from(amount),
        memo: send_memo.map(|m| Memo::try_from(m.as_str()).unwrap()),
    });
    let arguments = multisig::SubmitTransactionArgs {
        account,
        memo: memo.map(|x| Memo::try_from(x.as_str()).unwrap()),
        transaction: Box::new(transaction),
        threshold,
        timeout_in_secs: timeout.map(|d| d.as_secs()),
        execute_automatically,
        data_: None,
        memo_: legacy_memo.map(|x| MemoLegacy::try_from(x).unwrap()),
    };
    let response = client.call("account.multisigSubmitTransaction", arguments)?;

    let payload = crate::wait_response(client, response)?;
    let result: multisig::SubmitTransactionReturn = minicbor::decode(&payload)?;

    info!(
        "Transaction Token: {}",
        hex::encode(result.token.as_slice())
    );
    Ok(())
}

fn submit_set_defaults(
    client: ManyClient<impl Identity>,
    account: Address,
    multisig_arg: MultisigArgOpt,
    target: Address,
    opts: MultisigArgOpt,
) -> Result<(), ClientServerError> {
    let MultisigArgOpt {
        threshold,
        timeout,
        execute_automatically,
    } = multisig_arg;

    let transaction =
        events::AccountMultisigTransaction::AccountMultisigSetDefaults(multisig::SetDefaultsArgs {
            account: target,
            threshold: opts.threshold,
            timeout_in_secs: opts.timeout.map(|d| d.as_secs()),
            execute_automatically: opts.execute_automatically,
        });
    let arguments = multisig::SubmitTransactionArgs {
        account,
        memo: None,
        transaction: Box::new(transaction),
        threshold,
        timeout_in_secs: timeout.map(|d| d.as_secs()),
        execute_automatically,
        data_: None,
        memo_: None,
    };
    let response = client.call("account.multisigSubmitTransaction", arguments)?;

    let payload = crate::wait_response(client, response)?;
    let result: multisig::SubmitTransactionReturn = minicbor::decode(&payload)?;

    info!(
        "Transaction Token: {}",
        hex::encode(result.token.as_slice())
    );
    Ok(())
}

fn submit(
    client: ManyClient<impl Identity>,
    account: Address,
    multisig_arg: MultisigArgOpt,
    opts: SubmitOpt,
    memo: Option<String>,
    legacy_memo: Option<String>,
) -> Result<(), ClientServerError> {
    match opts {
        SubmitOpt::Send(target) => {
            submit_send(client, account, multisig_arg, target, memo, legacy_memo)
        }
        SubmitOpt::SetDefaults(SetDefaultsOpt {
            target_account,
            opts,
        }) => submit_set_defaults(client, account, multisig_arg, target_account, opts),
    }
}

fn approve(
    client: ManyClient<impl Identity>,
    opts: TransactionOpt,
) -> Result<(), ClientServerError> {
    let arguments = multisig::ApproveArgs { token: opts.token };
    let response = client.call("account.multisigApprove", arguments)?;

    let payload = crate::wait_response(client, response)?;
    let _result: multisig::ApproveReturn = minicbor::decode(&payload)?;

    info!("Approved.");

    Ok(())
}

fn revoke(
    client: ManyClient<impl Identity>,
    opts: TransactionOpt,
) -> Result<(), ClientServerError> {
    let arguments = multisig::RevokeArgs { token: opts.token };
    let response = client.call("account.multisigRevoke", arguments)?;

    let payload = crate::wait_response(client, response)?;
    let _result: multisig::RevokeReturn = minicbor::decode(&payload)?;

    info!("Revoked.");

    Ok(())
}

fn execute(
    client: ManyClient<impl Identity>,
    opts: TransactionOpt,
) -> Result<(), ClientServerError> {
    let arguments = multisig::ExecuteArgs { token: opts.token };
    let response = client.call("account.multisigExecute", arguments)?;

    let payload = crate::wait_response(client, response)?;
    let result: ResponseMessage = minicbor::decode(&payload)?;

    info!("Executed:");
    println!("{}", minicbor::display(&result.data?));
    Ok(())
}

fn info(client: ManyClient<impl Identity>, opts: TransactionOpt) -> Result<(), ClientServerError> {
    let arguments = multisig::InfoArgs { token: opts.token };
    let response = client.call("account.multisigInfo", arguments)?;

    let payload = crate::wait_response(client, response)?;
    let result: multisig::InfoReturn = minicbor::decode(&payload)?;

    println!("{result:#?}");
    Ok(())
}

fn set_defaults(
    client: ManyClient<impl Identity>,
    account: Address,
    opts: MultisigArgOpt,
) -> Result<(), ClientServerError> {
    let arguments = multisig::SetDefaultsArgs {
        account,
        threshold: opts.threshold,
        timeout_in_secs: opts.timeout.map(|d| d.as_secs()),
        execute_automatically: opts.execute_automatically,
    };
    let response = client.call("account.multisigSetDefaults", arguments)?;

    let payload = crate::wait_response(client, response)?;
    let _result: multisig::SetDefaultsReturn = minicbor::decode(&payload)?;

    info!("Defaults set.");
    Ok(())
}

pub fn multisig(
    client: ManyClient<impl Identity>,
    opts: CommandOpt,
) -> Result<(), ClientServerError> {
    match opts.subcommand {
        SubcommandOpt::Submit {
            account,
            multisig_arg,
            subcommand,
            memo,
            legacy_memo,
        } => submit(client, account, multisig_arg, subcommand, memo, legacy_memo),
        SubcommandOpt::Approve(sub_opts) => approve(client, sub_opts),
        SubcommandOpt::Revoke(sub_opts) => revoke(client, sub_opts),
        SubcommandOpt::Execute(sub_opts) => execute(client, sub_opts),
        SubcommandOpt::Info(sub_opts) => info(client, sub_opts),
        SubcommandOpt::SetDefaults(SetDefaultsOpt {
            target_account,
            opts,
        }) => set_defaults(client, target_account, opts),
    }
}
