use aggregator::{http::state::HttpState, test_utils::app::TestApp};
use alloy::{node_bindings::Anvil, signers::k256::elliptic_curve::rand_core::OsRng};
use utils::{
    eigen_client::EigenClient,
    eth_client::{EthClientBuilder, EthClientConfig},
    hello_world::{
        solidity_types::hello_world::HelloWorldServiceManager::NewTaskCreated,
        HelloWorldFullClientBuilder,
    },
};

const ANVIL_DEFAULT_MNEMONIC: &str = "test test test test test test test test test test test junk";

#[tokio::test]
async fn submit_to_chain() {
    tracing::info!("Running e2e aggregator");
    let anvil = Anvil::new().spawn();
    let aggregator = TestApp::new_with_args(aggregator::args::CliArgs {
        ws_endpoint: Some(anvil.ws_endpoint()),
        http_endpoint: Some(anvil.endpoint()),
        ..TestApp::default_cli_args()
    });
    let eth_client = EthClientBuilder::new(EthClientConfig {
        ws_endpoint: anvil.ws_endpoint(),
        http_endpoint: anvil.endpoint(),
        mnemonic: Some(ANVIL_DEFAULT_MNEMONIC.to_owned()),
        hd_index: None,
    })
    .build_signing()
    .await
    .unwrap();
    let eigen_client = EigenClient::new(eth_client);
    let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

    let hello_world_client = HelloWorldFullClientBuilder::new(eigen_client.eth.clone())
        .avs_addresses(core_contracts.clone())
        .build()
        .await
        .unwrap();

    // Register operator
    eigen_client
        .register_operator(&core_contracts)
        .await
        .unwrap();
    hello_world_client
        .register_operator(&mut OsRng)
        .await
        .unwrap();

    let hello_world_client = hello_world_client.into_simple();
    let task_message = "world".to_owned();

    let NewTaskCreated { task, taskIndex } = hello_world_client
        .create_new_task(task_message)
        .await
        .unwrap();

    let request = hello_world_client
        .task_request(task, taskIndex)
        .await
        .unwrap();

    let state = HttpState::new((*aggregator.config).clone()).unwrap();
    let _ = aggregator::http::handlers::service::add_task::add_task(state, request)
        .await
        .unwrap();
}
