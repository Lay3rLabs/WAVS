use std::{sync::Arc, time::Duration};

use wavs::AppContext;
use wavs_cli::clients::HttpClient;

use super::config::Configs;

#[derive(Clone)]
pub struct Clients {
    pub http_client: HttpClient,
    pub cli_ctx: Arc<wavs_cli::context::CliContext>,
}

impl Clients {
    pub fn new(ctx: AppContext, configs: &Configs) -> Self {
        ctx.rt.block_on(async {
            let http_client = HttpClient::new(&configs.cli);

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

            let cli_ctx = wavs_cli::context::CliContext::new_chains(
                configs.cli_args.clone(),
                configs.chains.all_chain_names(),
                configs.cli.clone(),
                None,
            )
            .await
            .unwrap();

            Self {
                http_client,
                cli_ctx: Arc::new(cli_ctx),
            }
        })
    }
}
