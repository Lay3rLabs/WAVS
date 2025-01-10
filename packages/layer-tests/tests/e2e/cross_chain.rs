use wavs::config::Config;

use super::{http::HttpClient, Digests, ServiceIds};

pub async fn run_tests_crosschain(
    _http_client: HttpClient,
    _config: Config,
    _digests: Digests,
    _service_ids: ServiceIds,
) {
    tracing::info!("Running e2e crosschain tests");
    // TODO!
}
