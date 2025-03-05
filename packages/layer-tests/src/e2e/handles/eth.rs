use std::process::{Command, Stdio};

use alloy::providers::Provider;
use utils::{config::EthereumChainConfig, context::AppContext, eth_client::EthClientBuilder};

use crate::e2e::config::Configs;

pub struct EthereumInstance {
    _anvil: LameAnvilInstance,
    _chain_config: EthereumChainConfig,
}

impl EthereumInstance {
    pub fn spawn(ctx: AppContext, configs: &Configs, chain_config: EthereumChainConfig) -> Self {
        let port = chain_config
            .http_endpoint
            .as_ref()
            .unwrap_or_else(|| chain_config.ws_endpoint.as_ref().unwrap())
            .split(':')
            .last()
            .unwrap()
            .parse::<u16>()
            .unwrap();

        tracing::info!(
            "Starting Ethereum chain: {} on port {}",
            chain_config.chain_id,
            port
        );

        // Something is broken with Alloy's anvil thing... let's use our own
        let anvil = LameAnvilInstanceBuilder {
            port,
            chain_id: chain_config.chain_id.clone(),
            block_time: configs.anvil_interval_seconds,
        }
        .spawn();

        // if we don't have an explicit interval, alloy will move blocks forward by transaction
        // otherwise, we should wait to get a new block so we can be sure anvil is up and running fully
        if let Some(interval_seconds) = configs.anvil_interval_seconds {
            ctx.rt.block_on(async {
                let client = EthClientBuilder::new(chain_config.to_client_config(None, None, None))
                    .build_query()
                    .await
                    .unwrap();

                let block = client.provider.get_block_number().await.unwrap();

                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(interval_seconds)).await;
                    if client.provider.get_block_number().await.unwrap() > block {
                        break;
                    }
                }
            })
        }

        Self {
            _anvil: anvil,
            _chain_config: chain_config,
        }
    }
}

// TODO: keep an eye on this
// Latest Alloy breaks things... not sure why.
// but their code doesn't really do anything more than what we have here, is just a bit more opinionated and parses the output
// and this way we have more control, so probably leave it as it is, at least until it breaks again :P

struct LameAnvilInstanceBuilder {
    pub port: u16,
    pub chain_id: String,
    pub block_time: Option<u64>,
}

impl LameAnvilInstanceBuilder {
    pub fn spawn(self) -> LameAnvilInstance {
        let mut args = vec![
            "-p".to_string(),
            self.port.to_string(),
            "--chain-id".to_string(),
            self.chain_id,
        ];

        if let Some(block_time) = self.block_time {
            args.push("-b".to_string());
            args.push(block_time.to_string());
        }

        let child = Command::new("anvil")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();

        LameAnvilInstance { child }
    }
}

struct LameAnvilInstance {
    child: std::process::Child,
}

impl Drop for LameAnvilInstance {
    fn drop(&mut self) {
        if let Err(err) = self.child.kill() {
            tracing::error!("Failed to kill anvil: {}", err);
        }
    }
}

impl Drop for EthereumInstance {
    fn drop(&mut self) {
        tracing::info!("Stopping Ethereum chain: {}", self._chain_config.chain_id);
    }
}
