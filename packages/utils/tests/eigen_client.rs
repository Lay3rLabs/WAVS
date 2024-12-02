use std::path::PathBuf;

use alloy::node_bindings::Anvil;
use utils::{
    eigen_client::{config::EigenClientConfig, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
};

#[tokio::test]
async fn register_operator() {
    let anvil = Anvil::new().block_time(1).try_spawn().unwrap();

    let config = EthClientConfig {
        ws_endpoint: anvil.ws_endpoint().to_string(),
        http_endpoint: anvil.endpoint().to_string(),
        mnemonic: Some(
            "work man father plunge mystery proud hollow address reunion sauce theory bonus"
                .to_string(),
        ),
    };

    let builder = EthClientBuilder::new(config);
    let eth_client = builder.build_signing().await.unwrap();

    let deployments_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("deployments");

    let core_deployment_data =
        tokio::fs::read_to_string(deployments_dir.join("core").join("31337.json"))
            .await
            .unwrap();

    let hello_world_deployment_data =
        tokio::fs::read_to_string(deployments_dir.join("hello-world").join("31337.json"))
            .await
            .unwrap();

    let eigen_config = EigenClientConfig {
        core: serde_json::from_str(&core_deployment_data).unwrap(),
        avs: serde_json::from_str(&hello_world_deployment_data).unwrap(),
    };

    let eigen_client = EigenClient::new(eth_client, eigen_config);

    eigen_client.register_operator().await.unwrap();
}
