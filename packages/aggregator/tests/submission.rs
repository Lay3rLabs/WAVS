use std::path::PathBuf;

use aggregator::{http::state::HttpState, test_utils::app::TestApp};
use alloy::{
    node_bindings::{Anvil, AnvilInstance},
    primitives::{eip191_hash_message, keccak256},
    signers::{
        k256::elliptic_curve::rand_core::OsRng,
        local::{coins_bip39::English, MnemonicBuilder},
        SignerSync,
    },
    sol_types::SolValue,
};
use utils::{
    aggregator::{AddAggregatorServiceRequest, AggregateAvsResponse},
    eigen_client::EigenClient,
    eth_client::{EthClientBuilder, EthClientConfig},
    layer_contract_client::{
        layer_service_manager::ILayerServiceManager::Payload, LayerContractClientFullBuilder,
        LayerContractClientSimple,
    },
};

const ANVIL_DEFAULT_MNEMONIC: &str = "test test test test test test test test test test test junk";

fn setup_anvil() -> (AnvilInstance, PathBuf) {
    let anvil = Anvil::new().spawn();
    let data_path = tempfile::tempdir().unwrap().path().to_path_buf();
    // host
    let file_contents = include_str!("../aggregator.toml");
    let _ = utils::storage::fs::FileStorage::new(data_path.clone());
    let new_temp_file = data_path.join("aggregator.toml");
    std::fs::File::create(&new_temp_file).unwrap();
    std::fs::write(&new_temp_file, file_contents).unwrap();

    // modify the `chains.eth.local` ws_endpoint to the anvil ws_endpoint
    let toml_content = std::fs::read_to_string(data_path.join("aggregator.toml")).unwrap();
    let toml_content = toml_content
        .replace("ws://localhost:8545", &anvil.ws_endpoint())
        .replace("http://localhost:8545", &anvil.endpoint())
        .replace("31337", &anvil.chain_id().to_string());
    std::fs::write(data_path.join("aggregator.toml"), toml_content).unwrap();

    (anvil, data_path)
}

#[tokio::test]
async fn submit_to_chain() {
    let (anvil, data_path) = setup_anvil();

    let _ = utils::storage::fs::FileStorage::new(data_path.clone());
    let aggregator = TestApp::new_with_args(aggregator::args::CliArgs {
        home: Some(data_path.clone()),
        data: Some(data_path),
        ..TestApp::default_cli_args()
    });
    let eth_client = EthClientBuilder::new(EthClientConfig {
        chain_id: anvil.chain_id().to_string(),
        ws_endpoint: Some(anvil.ws_endpoint()),
        http_endpoint: anvil.endpoint(),
        mnemonic: Some(ANVIL_DEFAULT_MNEMONIC.to_owned()),
        hd_index: None,
        transport: None,
    })
    .build_signing()
    .await
    .unwrap();
    let eigen_client = EigenClient::new(eth_client.clone());
    let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

    let avs_client = LayerContractClientFullBuilder::new(eigen_client.eth.clone())
        .avs_addresses(core_contracts.clone())
        .build()
        .await
        .unwrap();

    // Register operator
    eigen_client
        .register_operator(&core_contracts)
        .await
        .unwrap();

    avs_client.register_operator(&mut OsRng).await.unwrap();

    let avs_client: LayerContractClientSimple = avs_client.into();

    let state = HttpState::new((*aggregator.config).clone()).unwrap();

    aggregator::http::handlers::service::add_service::add_service(
        state.clone(),
        AddAggregatorServiceRequest::EthTrigger {
            service_manager_address: avs_client.service_manager_contract_address,
            service_id: "default".to_string(),
        },
    )
    .await
    .unwrap();

    let task_message = b"world".to_vec();

    let trigger_id = avs_client
        .trigger
        .add_trigger("default", "default", task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(trigger_id, task_message)
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_payload::add_payload_trigger(
        state,
        anvil.chain_id().to_string(),
        signed_payload,
        avs_client.service_manager_contract_address,
        "default".to_string(),
    )
    .await
    .unwrap();

    match response {
        AggregateAvsResponse::Sent { count, .. } => {
            assert!(count > 0);
        }
        _ => {
            panic!("Expected sent response");
        }
    }

    // Ensure it's landed!
    avs_client
        .load_signed_data(trigger_id)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn submit_to_chain_three() {
    let (anvil, data_path) = setup_anvil();

    let aggregator = TestApp::new_with_args(aggregator::args::CliArgs {
        // ws_endpoint: Some(anvil.ws_endpoint()),
        // http_endpoint: Some(anvil.endpoint()),
        tasks_quorum: Some(3),
        data: Some(data_path.clone()),
        home: Some(data_path),
        ..TestApp::default_cli_args()
    });
    let eth_client = EthClientBuilder::new(EthClientConfig {
        chain_id: anvil.chain_id().to_string(),
        ws_endpoint: Some(anvil.ws_endpoint()),
        http_endpoint: anvil.endpoint(),
        mnemonic: Some(ANVIL_DEFAULT_MNEMONIC.to_owned()),
        hd_index: None,
        transport: None,
    })
    .build_signing()
    .await
    .unwrap();
    let eigen_client = EigenClient::new(eth_client);
    let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

    let avs_client = LayerContractClientFullBuilder::new(eigen_client.eth.clone())
        .avs_addresses(core_contracts.clone())
        .build()
        .await
        .unwrap();

    // Register operator
    eigen_client
        .register_operator(&core_contracts)
        .await
        .unwrap();

    avs_client.register_operator(&mut OsRng).await.unwrap();

    let avs_client: LayerContractClientSimple = avs_client.into();

    let state = HttpState::new((*aggregator.config).clone()).unwrap();

    aggregator::http::handlers::service::add_service::add_service(
        state.clone(),
        AddAggregatorServiceRequest::EthTrigger {
            service_manager_address: avs_client.service_manager_contract_address,
            service_id: "default".to_string(),
        },
    )
    .await
    .unwrap();

    // first task - should just aggregate
    let task_message = b"foo".to_vec();

    let trigger_id = avs_client
        .trigger
        .add_trigger("default", "default", task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(trigger_id, task_message)
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_payload::add_payload_trigger(
        state.clone(),
        anvil.chain_id().to_string(),
        signed_payload,
        avs_client.service_manager_contract_address,
        "default".to_string(),
    )
    .await
    .unwrap();

    assert!(matches!(
        response,
        AggregateAvsResponse::Aggregated { count: 1 }
    ));

    // Second - still aggregating
    let task_message = b"hello".to_vec();

    let trigger_id = avs_client
        .trigger
        .add_trigger("default", "default", task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(trigger_id, task_message)
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_payload::add_payload_trigger(
        state.clone(),
        anvil.chain_id().to_string(),
        signed_payload,
        avs_client.service_manager_contract_address,
        "default".to_string(),
    )
    .await
    .unwrap();

    assert!(matches!(
        response,
        AggregateAvsResponse::Aggregated { count: 2 }
    ));

    // Third should get to the quorum
    let task_message = b"world".to_vec();

    let trigger_id = avs_client
        .trigger
        .add_trigger("default", "default", task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(trigger_id, task_message)
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_payload::add_payload_trigger(
        state.clone(),
        anvil.chain_id().to_string(),
        signed_payload,
        avs_client.service_manager_contract_address,
        "default".to_string(),
    )
    .await
    .unwrap();

    match response {
        AggregateAvsResponse::Sent { count, .. } => {
            assert_eq!(count, 3);
        }
        AggregateAvsResponse::Aggregated { count } => {
            panic!(
                "Expected sent response, instead got aggregated with count {}",
                count
            );
        }
    }

    // Ensure it's landed!
    avs_client
        .load_signed_data(trigger_id)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn invalid_operator_signature() {
    let (anvil, data_path) = setup_anvil();

    let aggregator = TestApp::new_with_args(aggregator::args::CliArgs {
        // ws_endpoint: Some(anvil.ws_endpoint()),
        // http_endpoint: Some(anvil.endpoint()),
        data: Some(data_path.clone()),
        home: Some(data_path),
        ..TestApp::default_cli_args()
    });
    let eth_client = EthClientBuilder::new(EthClientConfig {
        chain_id: anvil.chain_id().to_string(),
        ws_endpoint: Some(anvil.ws_endpoint()),
        http_endpoint: anvil.endpoint(),
        mnemonic: Some(ANVIL_DEFAULT_MNEMONIC.to_owned()),
        hd_index: None,
        transport: None,
    })
    .build_signing()
    .await
    .unwrap();
    let eigen_client = EigenClient::new(eth_client);
    let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

    let avs_client = LayerContractClientFullBuilder::new(eigen_client.eth.clone())
        .avs_addresses(core_contracts.clone())
        .build()
        .await
        .unwrap();

    // Register operator
    eigen_client
        .register_operator(&core_contracts)
        .await
        .unwrap();

    avs_client.register_operator(&mut OsRng).await.unwrap();

    let invalid_signer = MnemonicBuilder::<English>::default()
        .build_random_with(&mut OsRng)
        .unwrap();

    let avs_client: LayerContractClientSimple = avs_client.into();

    let state = HttpState::new((*aggregator.config).clone()).unwrap();
    aggregator::http::handlers::service::add_service::add_service(
        state.clone(),
        AddAggregatorServiceRequest::EthTrigger {
            service_manager_address: avs_client.service_manager_contract_address,
            service_id: "default".to_string(),
        },
    )
    .await
    .unwrap();

    let task_message = b"world".to_vec();

    let trigger_id = avs_client
        .trigger
        .add_trigger("default", "default", task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(trigger_id, task_message)
        .await
        .unwrap();

    // Invalid operator
    {
        let mut invalid_operator_payload = signed_payload.clone();
        invalid_operator_payload.operator = invalid_signer.address();
        let response = aggregator::http::handlers::service::add_payload::add_payload_trigger(
            state.clone(),
            anvil.chain_id().to_string(),
            invalid_operator_payload,
            avs_client.service_manager_contract_address,
            "default".to_string(),
        )
        .await
        .unwrap_err();
        assert!(format!("{response:?}").contains("Operator is not registered"));
    }

    // Invalid signature
    {
        let mut invalid_signature_payload = signed_payload.clone();
        let payload = Payload {
            triggerId: *trigger_id,
            data: signed_payload.data.into(),
        };

        let payload_hash = eip191_hash_message(keccak256(payload.abi_encode()));

        let signature = invalid_signer.sign_hash_sync(&payload_hash).unwrap();

        invalid_signature_payload.signature = signature;
        let response = aggregator::http::handlers::service::add_payload::add_payload_trigger(
            state,
            anvil.chain_id().to_string(),
            invalid_signature_payload,
            avs_client.service_manager_contract_address,
            "default".to_string(),
        )
        .await
        .unwrap_err();
        assert!(format!("{response:?}").contains("Operator signature does not match"));
    }
}
