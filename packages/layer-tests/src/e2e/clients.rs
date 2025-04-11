use std::collections::HashMap;
use std::{sync::Arc, time::Duration};

use alloy::primitives::utils::parse_ether;
use utils::context::AppContext;
use utils::eth_client::pool::{
    EthSigningClientFromPool, EthSigningClientPool, EthSigningClientPoolBuilder,
};
use wavs_cli::clients::HttpClient;
use wavs_types::ChainName;

use super::config::Configs;

#[derive(Clone)]
pub struct Clients {
    pub http_client: HttpClient,
    pub cli_ctx: Arc<wavs_cli::context::CliContext>,
    pub eth_client_pools: Arc<HashMap<ChainName, EthSigningClientPool>>,
}

impl Clients {
    pub fn new(ctx: AppContext, configs: &Configs) -> Self {
        ctx.rt.block_on(async {
            let http_client = HttpClient::new(configs.cli.wavs_endpoint.clone());

            // give the server a bit of time to start
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    match http_client.get_config().await {
                        Ok(_) => break,
                        Err(_) => {
                            tracing::info!("Waiting for server to start...");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            })
            .await
            .unwrap();

            // fund all the eth clients
            configs.mnemonics.fund(&configs.chains).await;

            let cli_ctx = wavs_cli::context::CliContext::new_chains(
                configs.cli_args.clone(),
                configs.chains.all_chain_names(),
                configs.cli.clone(),
                None,
            )
            .await
            .unwrap();

            let mut eth_client_pools = HashMap::new();

            // Create a pool for each Ethereum chain
            for (chain_name, chain_config) in &configs.chains.eth {
                let pool = EthSigningClientPoolBuilder::new(
                    None,
                    cli_ctx.config.eth_mnemonic.clone().unwrap(),
                    chain_config.clone(),
                )
                .with_initial_client_wei(parse_ether("1").unwrap())
                .build()
                .await
                .unwrap();

                eth_client_pools.insert(chain_name.clone(), pool);
            }

            Self {
                http_client,
                cli_ctx: Arc::new(cli_ctx),
                eth_client_pools: Arc::new(eth_client_pools),
            }
        })
    }

    // returns a deadpool managed EthSigningClient
    pub async fn get_eth_client(&self, chain_name: &ChainName) -> EthSigningClientFromPool {
        self.eth_client_pools
            .get(chain_name)
            .unwrap()
            .get()
            .await
            .unwrap()
    }
}
