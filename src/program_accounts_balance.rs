use std::{str::FromStr, sync::Arc, time::Duration};

use log::{error, info};
use solana_account_decoder::{UiAccountEncoding, UiDataSliceConfig};
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, RpcFilterType},
};
use solana_sdk::{native_token::lamports_to_sol, pubkey::Pubkey};
use tokio::{task::JoinHandle, time::sleep};

use crate::metrics::{remove_metric_total_balance_sol, update_metric_total_balance_sol};

const CHECK_INTERVAL: Duration = Duration::from_secs(300);
const BACKOFF_DURATION: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct ProgramAccountsBalanceConfig {
    name: String,
    program: Pubkey,
    filters: Vec<RpcFilterType>,
}

fn parse_rpc_filter_type(param: &str) -> anyhow::Result<RpcFilterType> {
    if let Some((key, value)) = param.split_once(':') {
        return Ok(match key {
            "b58" => parse_memcmp_base58_filter_type(value)?,
            "size" => parse_data_size_filter_type(value)?,
            _ => anyhow::bail!("Unsupported parameter type '{key}'"),
        });
    }
    anyhow::bail!("Malformed parameter '{param}'")
}

fn parse_memcmp_base58_filter_type(param: &str) -> anyhow::Result<RpcFilterType> {
    if let Some((offset, bytes)) = param.split_once(':') {
        return Ok(RpcFilterType::Memcmp(Memcmp::new(
            offset.parse()?,
            solana_client::rpc_filter::MemcmpEncodedBytes::Base58(bytes.into()),
        )));
    }
    anyhow::bail!("Failed to parse base58 memcmp filter from '{param}'")
}

fn parse_data_size_filter_type(data_size: &str) -> anyhow::Result<RpcFilterType> {
    Ok(RpcFilterType::DataSize(data_size.parse()?))
}

impl FromStr for ProgramAccountsBalanceConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, params) = match s.split_once('=') {
            Some((name, params)) => (name, params),
            None => anyhow::bail!(
                "Cannot parse ProgramAccountsBalanceConfig, expected syntax: name=params"
            ),
        };

        let mut params = params.split(' ').collect::<Vec<_>>().into_iter();

        let program = match params.next() {
            Some(program) => Pubkey::from_str(program)
                .expect(&format!("Failed to parse program ID from '{program}'")),
            None => anyhow::bail!("Program ID not found!"),
        };

        let mut filters = vec![];
        for param in params {
            filters.push(parse_rpc_filter_type(param)?);
        }

        Ok(ProgramAccountsBalanceConfig {
            name: name.to_string(),
            program,
            filters,
        })
    }
}

pub fn spawn_program_accounts_balance_watcher(
    rpc_client: Arc<RpcClient>,
    config: ProgramAccountsBalanceConfig,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!("Watching: {config:?}");
        loop {
            let response = rpc_client
                .get_program_accounts_with_config(
                    &config.program,
                    RpcProgramAccountsConfig {
                        filters: Some(config.filters.clone()),
                        account_config: RpcAccountInfoConfig {
                            data_slice: Some(UiDataSliceConfig {
                                offset: 0,
                                length: 0,
                            }),
                            encoding: Some(UiAccountEncoding::Base64),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
                .await;

            let response = match response {
                Ok(response) => response,
                Err(err) => {
                    error!("Failed to get RPC response: {err}");
                    remove_metric_total_balance_sol(&config.name);
                    sleep(BACKOFF_DURATION).await;
                    continue;
                }
            };

            let balance =
                lamports_to_sol(response.iter().map(|(_, account)| account.lamports).sum());
            update_metric_total_balance_sol(&config.name, balance);
            let count = response.len();
            info!(
                "For '{}' found {count} accounts with total balance: {balance}",
                config.name
            );

            sleep(CHECK_INTERVAL).await;
        }
    })
}
