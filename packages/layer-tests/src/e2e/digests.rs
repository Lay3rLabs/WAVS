use futures::{stream::FuturesUnordered, StreamExt};
use utils::filesystem::workspace_path;
use wavs::{AppContext, Digest};
use wavs_cli::clients::HttpClient;

use super::config::Configs;

#[derive(Debug, Clone, Default)]
pub struct Digests {
    pub chain_trigger_lookup: Option<Digest>,
    pub cosmos_query: Option<Digest>,
    pub echo_data: Option<Digest>,
    pub permissions: Option<Digest>,
    pub square: Option<Digest>,
}

impl Digests {
    pub fn new(ctx: AppContext, configs: &Configs, http_client: &HttpClient) -> Self {
        ctx.rt.block_on(async {
            let mut digests = Self::default();
            let matrix = &configs.test_config.matrix;

            let mut futures = FuturesUnordered::new();

            if matrix.eth.chain_trigger_lookup || matrix.cosmos.chain_trigger_lookup {
                futures.push(get_digest(http_client, "chain_trigger_lookup"));
            }

            if matrix.eth.cosmos_query || matrix.cosmos.cosmos_query {
                futures.push(get_digest(http_client, "cosmos_query"));
            }

            if matrix.eth.echo_data || matrix.cosmos.echo_data || matrix.eth.echo_data_aggregator {
                futures.push(get_digest(http_client, "echo_data"));
            }

            if matrix.eth.permissions || matrix.cosmos.permissions {
                futures.push(get_digest(http_client, "permissions"));
            }

            if matrix.eth.square || matrix.cosmos.square {
                futures.push(get_digest(http_client, "square"));
            }

            while let Some((name, digest)) = futures.next().await {
                match name {
                    "chain_trigger_lookup" => digests.chain_trigger_lookup = Some(digest),
                    "cosmos_query" => digests.cosmos_query = Some(digest),
                    "echo_data" => digests.echo_data = Some(digest),
                    "permissions" => digests.permissions = Some(digest),
                    "square" => digests.square = Some(digest),
                    _ => unreachable!(),
                }
            }

            digests
        })
    }
}

async fn get_digest(
    http_client: &HttpClient,
    wasm_filename: &'static str,
) -> (&'static str, Digest) {
    let wasm_path = workspace_path()
        .join("examples")
        .join("build")
        .join("components")
        .join(format!("{}.wasm", wasm_filename));

    tracing::info!("Uploading wasm: {}", wasm_path.display());

    let wasm_bytes = tokio::fs::read(wasm_path).await.unwrap();

    (
        wasm_filename,
        http_client
            .upload_component(wasm_bytes.to_vec())
            .await
            .unwrap(),
    )
}
