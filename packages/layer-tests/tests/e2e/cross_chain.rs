use wavs::config::Config;

use super::{cosmos::CosmosTestApp, eth::EthTestApp, http::HttpClient, Digests, ServiceIds};

pub async fn run_tests_crosschain(
    _eth_apps: Vec<EthTestApp>,
    _cosmos_apps: Vec<CosmosTestApp>,
    _http_client: HttpClient,
    _digests: Digests,
    _service_ids: ServiceIds,
) {
    tracing::info!("Running e2e crosschain tests");
    // TODO!
}
