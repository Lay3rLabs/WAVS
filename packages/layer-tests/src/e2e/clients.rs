use std::collections::HashMap;
use std::{sync::Arc, time::Duration};

use deadpool::managed::Pool;
use layer_climb::pool::{SigningClientPool, SigningClientPoolManager};
use utils::{config::EvmChainConfigExt, evm_client::EvmSigningClient};
use wavs_cli::clients::HttpClient;
use wavs_types::ChainKey;

use super::config::Configs;

#[derive(Clone)]
pub struct Clients {
    pub http_client: HttpClient,
    pub cli_ctx: Arc<wavs_cli::context::CliContext>,
    pub evm_clients: Arc<HashMap<ChainKey, EvmSigningClient>>,
    pub cosmos_client_pools: Arc<HashMap<ChainKey, SigningClientPool>>,
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

        let chains = { configs.chains.read().unwrap().clone() };

        // fund all the EVM clients
        configs.mnemonics.fund(&chains).await;

        let cli_ctx = wavs_cli::context::CliContext::new_deployment(
            configs.cli_args.clone(),
            configs.cli.clone(),
            None,
        )
        .await
        .unwrap();

        let mut evm_clients = HashMap::new();
        let mut cosmos_client_pools = HashMap::new();

        // Create a client for each EVM chain
        for chain_config in chains.evm_iter() {
            let client_config = chain_config
                .signing_client_config(cli_ctx.config.evm_credential.clone().unwrap())
                .unwrap();

            let evm_client = EvmSigningClient::new(client_config).await.unwrap();

            evm_clients.insert(chain_config.into(), evm_client);
        }

        // Create a client for each Cosmos chain
        for chain_config in chains.cosmos_iter() {
            let climb_chain_config = layer_climb::prelude::ChainConfig {
                chain_id: chain_config.chain_id.clone().into(),
                rpc_endpoint: chain_config.rpc_endpoint.clone(),
                grpc_endpoint: chain_config.grpc_endpoint.clone(),
                grpc_web_endpoint: None,
                gas_price: chain_config.gas_price,
                gas_denom: chain_config.gas_denom.clone(),
                address_kind: layer_climb::prelude::AddrKind::Cosmos {
                    prefix: chain_config.bech32_prefix.clone(),
                },
            };

            tracing::info!(
                "Setting up Cosmos client pool for {}",
                chain_config.chain_id
            );
            let pool_manager = SigningClientPoolManager::new_mnemonic(
                cli_ctx
                    .config
                    .cosmos_mnemonic
                    .clone()
                    .expect("Expected a cosmos mnemonic")
                    .to_string(),
                climb_chain_config,
                None,
                None,
            )
            .with_minimum_balance(10_000, 1_000_000, None, None)
            .await
            .unwrap();

            let pool =
                SigningClientPool::new(Pool::builder(pool_manager).max_size(8).build().unwrap());

            cosmos_client_pools.insert(chain_config.into(), pool);
        }

        Self {
            http_client,
            cli_ctx: Arc::new(cli_ctx),
            evm_clients: Arc::new(evm_clients),
            cosmos_client_pools: Arc::new(cosmos_client_pools),
        }
    }

    pub fn get_evm_client(&self, chain: &ChainKey) -> EvmSigningClient {
        match self.evm_clients.get(chain).cloned() {
            Some(client) => client,
            None => match self.cosmos_client_pools.get(chain).is_some() {
                false => panic!(
                    "No EVM or Cosmos client found for chain: {} (no Cosmos either, fwiw)",
                    chain
                ),
                true => {
                    panic!(
                        "No EVM client found for chain: {} (Cosmos client exists though... maybe you meant that?)",
                        chain
                    )
                }
            },
        }
    }

    pub async fn get_cosmos_client(
        &self,
        chain: &ChainKey,
    ) -> deadpool::managed::Object<SigningClientPoolManager> {
        match self.cosmos_client_pools.get(chain).unwrap().get().await {
            Ok(client) => client,
            Err(_) => match self.evm_clients.get(chain).is_some() {
                false => panic!(
                    "No Cosmos or EVM client found for chain: {} (no EVM either, fwiw)",
                    chain
                ),
                true => {
                    panic!(
                            "No Cosmos client found for chain: {} (EVM client exists though... maybe you meant that?)",
                            chain
                        )
                }
            },
        }
    }
}
