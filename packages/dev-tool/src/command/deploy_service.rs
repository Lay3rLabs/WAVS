use wavs_cli::clients::HttpClient;
use wavs_types::ComponentDigest;

use crate::service::{SERVICE, WAVS_COMPONENT_BYTES};

pub async fn run() {
    let client = HttpClient::new("http://127.0.0.1:8000".to_string());

    if client
        .upload_component(WAVS_COMPONENT_BYTES.clone())
        .await
        .unwrap()
        != ComponentDigest::hash(&*WAVS_COMPONENT_BYTES)
    {
        panic!("wavs component bytes got unexpected hash!");
    }

    client.dev_add_service_direct(&SERVICE).await.unwrap();
}
