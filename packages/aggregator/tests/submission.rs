use aggregator::{http::state::HttpState, test_utils::app::TestApp};
use alloy::{
    node_bindings::Anvil,
    primitives::keccak256,
    signers::{
        k256::elliptic_curve::rand_core::OsRng,
        local::{coins_bip39::English, MnemonicBuilder},
        SignerSync,
    },
    sol_types::SolValue,
};
use utils::{
    eigen_client::EigenClient,
    eth_client::{EthClientBuilder, EthClientConfig},
    hello_world::{
        solidity_types::hello_world::HelloWorldServiceManager::NewTaskCreated,
        AddAggregatorServiceRequest, HelloWorldFullClientBuilder,
    },
};

const ANVIL_DEFAULT_MNEMONIC: &str = "test test test test test test test test test test test junk";

#[tokio::test]
async fn submit_to_chain() {
    let anvil = Anvil::new().spawn();
    let data_path = tempfile::tempdir().unwrap().path().to_path_buf();
    let _ = utils::storage::fs::FileStorage::new(data_path.clone());
    let aggregator = TestApp::new_with_args(aggregator::args::CliArgs {
        ws_endpoint: Some(anvil.ws_endpoint()),
        http_endpoint: Some(anvil.endpoint()),
        data: Some(data_path),
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
    aggregator::http::handlers::service::add_service::add_service(
        state.clone(),
        AddAggregatorServiceRequest {
            service: hello_world_client.contract_address,
        },
    )
    .await
    .unwrap();

    let response = aggregator::http::handlers::service::add_task::add_task(state, request)
        .await
        .unwrap();
    assert!(response.hash.is_some());

    // Ensure it's landed!
    let response = hello_world_client
        .contract
        .allTaskResponses(hello_world_client.eth.address(), taskIndex)
        .call()
        .await
        .unwrap();
    assert!(!response._0.is_empty())
}

#[tokio::test]
async fn submit_to_chain_three() {
    let anvil = Anvil::new().spawn();
    let data_path = tempfile::tempdir().unwrap().path().to_path_buf();
    let _ = utils::storage::fs::FileStorage::new(data_path.clone());
    let aggregator = TestApp::new_with_args(aggregator::args::CliArgs {
        ws_endpoint: Some(anvil.ws_endpoint()),
        http_endpoint: Some(anvil.endpoint()),
        tasks_quorum: Some(3),
        data: Some(data_path),
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
    let state = HttpState::new((*aggregator.config).clone()).unwrap();
    aggregator::http::handlers::service::add_service::add_service(
        state.clone(),
        AddAggregatorServiceRequest {
            service: hello_world_client.contract_address,
        },
    )
    .await
    .unwrap();

    // First we just add task
    let task_message = "world".to_owned();
    let NewTaskCreated { task, taskIndex } = hello_world_client
        .create_new_task(task_message)
        .await
        .unwrap();
    let request = hello_world_client
        .task_request(task, taskIndex)
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_task::add_task(state.clone(), request)
        .await
        .unwrap();
    assert!(response.hash.is_none());

    // Second we just add as well
    let task_message = "bar".to_owned();
    let NewTaskCreated { task, taskIndex } = hello_world_client
        .create_new_task(task_message)
        .await
        .unwrap();

    let request = hello_world_client
        .task_request(task, taskIndex)
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_task::add_task(state.clone(), request)
        .await
        .unwrap();
    assert!(response.hash.is_none());

    // Third should get to the quorum
    let task_message = "bar".to_owned();
    let NewTaskCreated { task, taskIndex } = hello_world_client
        .create_new_task(task_message)
        .await
        .unwrap();

    let request = hello_world_client
        .task_request(task, taskIndex)
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_task::add_task(state, request)
        .await
        .unwrap();
    assert!(response.hash.is_some());

    // Ensure it's landed!
    let response = hello_world_client
        .contract
        .allTaskResponses(hello_world_client.contract_address, taskIndex)
        .call()
        .await
        .unwrap();
    assert!(!response._0.is_empty())
}

#[tokio::test]
async fn invalid_operator_signature() {
    let anvil = Anvil::new().spawn();
    let data_path = tempfile::tempdir().unwrap().path().to_path_buf();
    let _ = utils::storage::fs::FileStorage::new(data_path.clone());
    let aggregator = TestApp::new_with_args(aggregator::args::CliArgs {
        ws_endpoint: Some(anvil.ws_endpoint()),
        http_endpoint: Some(anvil.endpoint()),
        data: Some(data_path),
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
    let invalid_signer = MnemonicBuilder::<English>::default()
        .build_random_with(&mut OsRng)
        .unwrap();

    let hello_world_client = hello_world_client.into_simple();
    let task_message = "world".to_owned();

    let msg = keccak256(
        format!("Hello, {}", task_message)
            .abi_encode_packed()
            .as_slice(),
    );
    let invalid_operator = invalid_signer.address();
    let invalid_signature = invalid_signer.sign_message_sync(msg.as_ref()).unwrap();

    let NewTaskCreated { task, taskIndex } = hello_world_client
        .create_new_task(task_message)
        .await
        .unwrap();

    let request = hello_world_client
        .task_request(task, taskIndex)
        .await
        .unwrap();

    let state = HttpState::new((*aggregator.config).clone()).unwrap();
    aggregator::http::handlers::service::add_service::add_service(
        state.clone(),
        AddAggregatorServiceRequest {
            service: hello_world_client.contract_address,
        },
    )
    .await
    .unwrap();

    // Invalid operator
    {
        let mut invalid_operator_request = request.clone();
        invalid_operator_request.operator = invalid_operator;
        let response = aggregator::http::handlers::service::add_task::add_task(
            state.clone(),
            invalid_operator_request,
        )
        .await
        .unwrap_err();
        assert!(format!("{response:?}").contains("Operator is not registered"));
    }

    // Invalid signature
    {
        let mut invalid_signature_request = request.clone();
        invalid_signature_request.signature = invalid_signature.into();
        let response = aggregator::http::handlers::service::add_task::add_task(
            state,
            invalid_signature_request,
        )
        .await
        .unwrap_err();
        assert!(format!("{response:?}").contains("Operator signature does not match"));
    }
}
