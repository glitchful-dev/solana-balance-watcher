use clap::Parser;
use futures::future::join_all;
use log::info;
use solana_balance_watcher::{
    balance::spawn_balance_watcher,
    metrics::spawn_metrics_server,
    program_accounts_balance::{
        spawn_program_accounts_balance_watcher, ProgramAccountsBalanceConfig,
    },
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tracing_log::LogTracer;

#[derive(Debug, Parser)]
struct Flags {
    #[clap(long, required = true, env)]
    rpc_url: String,

    #[clap(long, required = true)]
    metrics_port: u16,

    #[arg(long = "named-address")]
    named_addresses: Vec<String>,

    #[arg(long = "program-accounts")]
    program_accounts_configs: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let flags: Flags = Flags::parse();
    LogTracer::init().expect("Logger setup failed");
    let subscriber = tracing_subscriber::fmt::Subscriber::builder()
        .with_target(false)
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_panic(info);
        log::error!("Worker thread panicked, exiting.");
        std::process::exit(1);
    }));

    let mut named_pubkeys: HashMap<Pubkey, String> = Default::default();

    for named_address in flags.named_addresses {
        if let Some((name, pubkey_str)) = named_address.split_once('=') {
            let pubkey = Pubkey::from_str(pubkey_str)
                .expect(&format!("Cannot parse pubkey from '{pubkey_str}'"));
            if let Some(previous_name) = named_pubkeys.get(&pubkey) {
                panic!("Trying to store pubkey '{pubkey}' with name '{name}' but it is stored with a different name '{previous_name}' already");
            }
            named_pubkeys.insert(pubkey, name.into());
            info!("Watching {name} ({pubkey})");
        } else {
            panic!("Failed to parse '{named_address}'");
        }
    }

    let rpc_client = Arc::new(RpcClient::new(flags.rpc_url));

    let mut handles = vec![];
    handles.push(spawn_metrics_server(flags.metrics_port));
    handles.push(spawn_balance_watcher(rpc_client.clone(), named_pubkeys));
    for program_account_config in flags.program_accounts_configs {
        handles.push(spawn_program_accounts_balance_watcher(
            rpc_client.clone(),
            ProgramAccountsBalanceConfig::from_str(&program_account_config)?,
        ));
    }

    join_all(handles).await;

    Ok(())
}
