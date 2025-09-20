use wavs_cli::clients::HttpClient;
use wavs_types::ComponentDigest;

use crate::service::{AGGREGATOR_COMPONENT_BYTES, SERVICE_MANAGER};

pub async fn run() {
    let client = HttpClient::new("http://127.0.0.1:8001".to_string());

    if client
        .upload_component(AGGREGATOR_COMPONENT_BYTES.clone())
        .await
        .unwrap()
        != ComponentDigest::hash(&*AGGREGATOR_COMPONENT_BYTES)
    {
        panic!("aggregator component bytes got unexpected hash!");
    }

    client
        .register_aggregator_service(&SERVICE_MANAGER)
        .await
        .unwrap();
}
