use std::collections::HashMap;
use std::{sync::Arc, time::Duration};

use utils::context::AppContext;
use utils::evm_client::EvmSigningClient;
use wavs_cli::clients::HttpClient;
use wavs_types::ChainName;

use super::config::Configs;

#[derive(Clone)]
pub struct Clients {
    pub http_client: HttpClient,
    pub cli_ctx: Arc<wavs_cli::context::CliContext>,
    pub evm_clients: Arc<HashMap<ChainName, EvmSigningClient>>,
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

            let mut cosmos_clients = HashMap::new();
            // Create a client for each Cosmos chain
            for chain_name in configs.chains.cosmos.keys() {
                let client = cli_ctx.new_cosmos_client(chain_name).await.unwrap();

                cosmos_clients.insert(chain_name.clone(), client);
            }

            Self {
                http_client,
                cli_ctx: Arc::new(cli_ctx),
                evm_clients: Arc::new(evm_clients),
                cosmos_clients: Arc::new(cosmos_clients),
            }
        })
    }

    pub fn get_evm_client(&self, chain_name: &ChainName) -> EvmSigningClient {
        self.evm_clients.get(chain_name).cloned().unwrap()
    }

    pub fn get_cosmos_client(&self, chain_name: &ChainName) -> layer_climb::prelude::SigningClient {
        self.cosmos_clients.get(chain_name).cloned().unwrap()
    }
}
