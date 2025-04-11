use std::collections::HashMap;
use std::{sync::Arc, time::Duration};

use utils::context::AppContext;
use utils::eth_client::{EthClientBuilder, EthClientTransport, EthSigningClient};
use wavs_cli::clients::HttpClient;
use wavs_types::ChainName;

use super::config::Configs;

#[derive(Clone)]
pub struct Clients {
    pub http_client: HttpClient,
    pub cli_ctx: Arc<wavs_cli::context::CliContext>,
    pub eth_clients: Arc<HashMap<ChainName, EthSigningClient>>,
    pub cosmos_clients: Arc<HashMap<ChainName, layer_climb::prelude::SigningClient>>,
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

            let cli_ctx = wavs_cli::context::CliContext::new_deployment(
                configs.cli_args.clone(),
                configs.cli.clone(),
                None,
            )
            .await
            .unwrap();

            let mut eth_clients = HashMap::new();

            // Create a pool for each Ethereum chain
            for (chain_name, chain_config) in &configs.chains.eth {
                let client = EthClientBuilder::new(chain_config.to_client_config(
                    None,
                    cli_ctx.config.eth_mnemonic.clone(),
                    Some(EthClientTransport::Http),
                ))
                .build_signing()
                .await
                .unwrap();

                eth_clients.insert(chain_name.clone(), client);
            }

            let mut cosmos_clients = HashMap::new();
            // Create a client for each Cosmos chain
            for chain_name in configs.chains.cosmos.keys() {
                let client = cli_ctx.new_cosmos_client(chain_name).await.unwrap();

                cosmos_clients.insert(chain_name.clone(), client);
            }

            Self {
                http_client,
                cli_ctx: Arc::new(cli_ctx),
                eth_clients: Arc::new(eth_clients),
                cosmos_clients: Arc::new(cosmos_clients),
            }
        })
    }

    // returns a deadpool managed EthSigningClient
    pub async fn get_eth_client(&self, chain_name: &ChainName) -> EthSigningClient {
        self.eth_clients.get(chain_name).cloned().unwrap()
    }

    // for now, just returns a cosmos client with a simple cache
    pub fn get_cosmos_client(&self, chain_name: &ChainName) -> layer_climb::prelude::SigningClient {
        self.cosmos_clients.get(chain_name).cloned().unwrap()
    }
}
