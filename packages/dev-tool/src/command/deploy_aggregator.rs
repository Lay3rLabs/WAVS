use wavs_cli::clients::HttpClient;
use wavs_types::ComponentDigest;

use crate::service::{AGGREGATOR_COMPONENT_BYTES, SERVICE};

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

    let service_manager = &SERVICE.manager;
    client
        .register_aggregator_service(service_manager)
        .await
        .unwrap();
}
