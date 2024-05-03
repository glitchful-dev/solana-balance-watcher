use std::{collections::HashMap, sync::Arc, time::Duration};

use log::{error, info};
use solana_account_decoder::UiDataSliceConfig;
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcAccountInfoConfig};
use solana_sdk::{native_token::lamports_to_sol, pubkey::Pubkey};
use tokio::{task::JoinHandle, time::sleep};

use crate::metrics::{reset_metric_balance_sol, update_metric_balance_sol};

const CHECK_INTERVAL: Duration = Duration::from_secs(300);
const BACKOFF_DURATION: Duration = Duration::from_secs(10);

pub fn spawn_balance_watcher(
    rpc_client: Arc<RpcClient>,
    named_pubkeys: HashMap<Pubkey, String>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let pubkeys: Vec<_> = named_pubkeys.keys().cloned().collect();
        loop {
            let response = rpc_client
                .get_multiple_accounts_with_config(
                    pubkeys.as_slice(),
                    RpcAccountInfoConfig {
                        data_slice: Some(UiDataSliceConfig {
                            offset: 0,
                            length: 0,
                        }),
                        ..Default::default()
                    },
                )
                .await;

            let response = match response {
                Ok(response) => response,
                Err(err) => {
                    error!("Failed to get RPC response: {err}");
                    reset_metric_balance_sol();
                    sleep(BACKOFF_DURATION).await;
                    continue;
                }
            };

            for (pubkey, account) in pubkeys.iter().zip(response.value.into_iter()) {
                if let None = account {
                    error!("Account {pubkey} does not exist");
                }

                let balance = lamports_to_sol(account.map(|a| a.lamports).unwrap_or(0));
                info!("Balance {pubkey}: {balance}");
                update_metric_balance_sol(
                    named_pubkeys.get(pubkey).unwrap(),
                    &pubkey.to_string(),
                    balance,
                );
            }

            sleep(CHECK_INTERVAL).await;
        }
    })
}
