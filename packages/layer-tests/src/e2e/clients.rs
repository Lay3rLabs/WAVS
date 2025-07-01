use std::collections::HashMap;
use std::{sync::Arc, time::Duration};

use deadpool::managed::Pool;
use layer_climb::pool::{SigningClientPool, SigningClientPoolManager};
use utils::{config::EvmChainConfigExt, evm_client::EvmSigningClient};
use wavs_cli::clients::HttpClient;
use wavs_types::ChainName;

use super::config::Configs;

#[derive(Clone)]
pub struct Clients {
    pub http_client: HttpClient,
    pub cli_ctx: Arc<wavs_cli::context::CliContext>,
    pub evm_clients: Arc<HashMap<ChainName, EvmSigningClient>>,
    pub cosmos_client_pools: Arc<HashMap<ChainName, SigningClientPool>>,
}

impl Clients {
    pub async fn new(configs: &Configs) -> Self {
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

        // fund all the EVM clients
        configs.mnemonics.fund(&configs.chains).await;

        let cli_ctx = wavs_cli::context::CliContext::new_deployment(
            configs.cli_args.clone(),
            configs.cli.clone(),
            None,
        )
        .await
        .unwrap();

        let mut evm_clients = HashMap::new();

        // Create a client for each EVM chain
        for (chain_name, chain_config) in &configs.chains.evm {
            let client_config = chain_config
                .signing_client_config(cli_ctx.config.evm_credential.clone().unwrap())
                .unwrap();

            let evm_client = EvmSigningClient::new(client_config).await.unwrap();

            evm_clients.insert(chain_name.clone(), evm_client);
        }

        let mut cosmos_client_pools = HashMap::new();
        // Create a client for each Cosmos chain
        for (chain_name, chain_config) in &configs.chains.cosmos {
            let chain_config = layer_climb::prelude::ChainConfig {
                chain_id: layer_climb::prelude::ChainId::new(chain_config.chain_id.clone()),
                rpc_endpoint: chain_config.rpc_endpoint.clone(),
                grpc_endpoint: chain_config.grpc_endpoint.clone(),
                grpc_web_endpoint: None,
                gas_price: chain_config.gas_price,
                gas_denom: chain_config.gas_denom.clone(),
                address_kind: layer_climb::prelude::AddrKind::Cosmos {
                    prefix: chain_config.bech32_prefix.clone(),
                },
            };

            let pool_manager = SigningClientPoolManager::new_mnemonic(
                cli_ctx
                    .config
                    .cosmos_mnemonic
                    .clone()
                    .expect("Expected a cosmos mnemonic"),
                chain_config,
                None,
                None,
            )
            .with_minimum_balance(10_000, 1_000_000, None, None)
            .await
            .unwrap();

            let pool =
                SigningClientPool::new(Pool::builder(pool_manager).max_size(8).build().unwrap());

            cosmos_client_pools.insert(chain_name.clone(), pool);
        }

        Self {
            http_client,
            cli_ctx: Arc::new(cli_ctx),
            evm_clients: Arc::new(evm_clients),
            cosmos_client_pools: Arc::new(cosmos_client_pools),
        }
    }

    pub fn get_evm_client(&self, chain_name: &ChainName) -> EvmSigningClient {
        self.evm_clients.get(chain_name).cloned().unwrap()
    }

    pub async fn get_cosmos_client(
        &self,
        chain_name: &ChainName,
    ) -> deadpool::managed::Object<SigningClientPoolManager> {
        self.cosmos_client_pools
            .get(chain_name)
            .unwrap()
            .get()
            .await
            .unwrap()
    }
}
