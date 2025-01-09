use aggregator::{http::state::HttpState, test_utils::app::TestApp};
use alloy::{
    node_bindings::Anvil,
    primitives::{eip191_hash_message, keccak256},
    signers::{
        k256::elliptic_curve::rand_core::OsRng,
        local::{coins_bip39::English, MnemonicBuilder},
        SignerSync,
    },
};
use utils::{
    aggregator::{AddAggregatorServiceRequest, AggregateAvsResponse},
    avs_client::{AvsClientBuilder, ServiceManagerClient},
    eigen_client::EigenClient,
    example_client::{SimpleSubmitClient, SimpleTriggerClient},
};

#[tokio::test]
async fn submit_to_chain() {
    let anvil = Anvil::new().spawn();
    let data_path = tempfile::tempdir().unwrap().path().to_path_buf();
    let _ = utils::storage::fs::FileStorage::new(data_path.clone());
    let aggregator = TestApp::new_with_args(
        aggregator::args::CliArgs {
            chain: Some("local".to_string()),
            data: Some(data_path),
            ..TestApp::zeroed_cli_args()
        },
        Some(&anvil),
    );
    let eth_client = aggregator.eth_signing_client().await;
    let eigen_client = EigenClient::new(eth_client);
    let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

    let avs_client = AvsClientBuilder::new(eigen_client.eth.clone())
        .core_addresses(core_contracts.clone())
        .build(SimpleSubmitClient::deploy)
        .await
        .unwrap();

    let submit_client =
        SimpleSubmitClient::new(avs_client.eth.clone(), avs_client.layer.service_manager);

    // Register operator
    eigen_client
        .register_operator(&core_contracts)
        .await
        .unwrap();

    avs_client.register_operator(&mut OsRng).await.unwrap();

    let avs_client: ServiceManagerClient = avs_client.into();

    let state = HttpState::new((*aggregator.config).clone()).unwrap();

    aggregator::http::handlers::service::add_service::add_service(
        state.clone(),
        AddAggregatorServiceRequest::EthTrigger {
            service_manager_address: avs_client.service_manager_contract_address,
        },
    )
    .await
    .unwrap();

    let trigger_client = SimpleTriggerClient::new_deploy(avs_client.eth.clone())
        .await
        .unwrap();
    let task_message = b"world".to_vec();

    let trigger_id = trigger_client
        .add_trigger(task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(SimpleSubmitClient::data_with_id_bytes(
            *trigger_id,
            task_message,
        ))
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_payload::add_payload(
        state,
        signed_payload,
        avs_client.service_manager_contract_address,
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
    assert!(submit_client.trigger_validated(trigger_id).await);
}

#[tokio::test]
async fn submit_to_chain_three() {
    let anvil = Anvil::new().spawn();
    let data_path = tempfile::tempdir().unwrap().path().to_path_buf();
    let _ = utils::storage::fs::FileStorage::new(data_path.clone());
    let aggregator = TestApp::new_with_args(
        aggregator::args::CliArgs {
            tasks_quorum: Some(3),
            chain: Some("local".to_string()),
            data: Some(data_path),
            ..TestApp::zeroed_cli_args()
        },
        Some(&anvil),
    );
    let eth_client = aggregator.eth_signing_client().await;
    let eigen_client = EigenClient::new(eth_client);

    let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

    let avs_client = AvsClientBuilder::new(eigen_client.eth.clone())
        .core_addresses(core_contracts.clone())
        .build(SimpleSubmitClient::deploy)
        .await
        .unwrap();

    let submit_client =
        SimpleSubmitClient::new(avs_client.eth.clone(), avs_client.layer.service_manager);

    // Register operator
    eigen_client
        .register_operator(&core_contracts)
        .await
        .unwrap();

    avs_client.register_operator(&mut OsRng).await.unwrap();

    let avs_client: ServiceManagerClient = avs_client.into();

    let state = HttpState::new((*aggregator.config).clone()).unwrap();

    aggregator::http::handlers::service::add_service::add_service(
        state.clone(),
        AddAggregatorServiceRequest::EthTrigger {
            service_manager_address: avs_client.service_manager_contract_address,
        },
    )
    .await
    .unwrap();

    // first task - should just aggregate
    let task_message = b"foo".to_vec();

    let trigger_client = SimpleTriggerClient::new_deploy(avs_client.eth.clone())
        .await
        .unwrap();

    let trigger_id = trigger_client
        .add_trigger(task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(SimpleSubmitClient::data_with_id_bytes(
            *trigger_id,
            task_message,
        ))
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_payload::add_payload(
        state.clone(),
        signed_payload,
        avs_client.service_manager_contract_address,
    )
    .await
    .unwrap();

    assert!(matches!(
        response,
        AggregateAvsResponse::Aggregated { count: 1 }
    ));

    // Second - still aggregating
    let task_message = b"hello".to_vec();

    let trigger_id = trigger_client
        .add_trigger(task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(SimpleSubmitClient::data_with_id_bytes(
            *trigger_id,
            task_message,
        ))
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_payload::add_payload(
        state.clone(),
        signed_payload,
        avs_client.service_manager_contract_address,
    )
    .await
    .unwrap();

    assert!(matches!(
        response,
        AggregateAvsResponse::Aggregated { count: 2 }
    ));

    // Third should get to the quorum
    let task_message = b"world".to_vec();

    let trigger_id = trigger_client
        .add_trigger(task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(SimpleSubmitClient::data_with_id_bytes(
            *trigger_id,
            task_message,
        ))
        .await
        .unwrap();

    let response = aggregator::http::handlers::service::add_payload::add_payload(
        state.clone(),
        signed_payload,
        avs_client.service_manager_contract_address,
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
    assert!(submit_client.trigger_validated(trigger_id).await);
}

#[tokio::test]
async fn invalid_operator_signature() {
    let anvil = Anvil::new().spawn();
    let data_path = tempfile::tempdir().unwrap().path().to_path_buf();
    let _ = utils::storage::fs::FileStorage::new(data_path.clone());
    let aggregator = TestApp::new_with_args(
        aggregator::args::CliArgs {
            chain: Some("local".to_string()),
            data: Some(data_path),
            ..TestApp::zeroed_cli_args()
        },
        Some(&anvil),
    );
    let eth_client = aggregator.eth_signing_client().await;
    let eigen_client = EigenClient::new(eth_client);
    let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

    let avs_client = AvsClientBuilder::new(eigen_client.eth.clone())
        .core_addresses(core_contracts.clone())
        .build(SimpleSubmitClient::deploy)
        .await
        .unwrap();

    let _submit_client =
        SimpleSubmitClient::new(avs_client.eth.clone(), avs_client.layer.service_manager);

    // Register operator
    eigen_client
        .register_operator(&core_contracts)
        .await
        .unwrap();

    avs_client.register_operator(&mut OsRng).await.unwrap();

    let invalid_signer = MnemonicBuilder::<English>::default()
        .build_random_with(&mut OsRng)
        .unwrap();

    let avs_client: ServiceManagerClient = avs_client.into();

    let state = HttpState::new((*aggregator.config).clone()).unwrap();
    aggregator::http::handlers::service::add_service::add_service(
        state.clone(),
        AddAggregatorServiceRequest::EthTrigger {
            service_manager_address: avs_client.service_manager_contract_address,
        },
    )
    .await
    .unwrap();

    let trigger_client = SimpleTriggerClient::new_deploy(avs_client.eth.clone())
        .await
        .unwrap();

    let task_message = b"world".to_vec();

    let trigger_id = trigger_client
        .add_trigger(task_message.clone())
        .await
        .unwrap();

    let signed_payload = avs_client
        .sign_payload(SimpleSubmitClient::data_with_id_bytes(
            *trigger_id,
            task_message,
        ))
        .await
        .unwrap();

    // Invalid operator
    {
        let mut invalid_operator_payload = signed_payload.clone();
        invalid_operator_payload.operator = invalid_signer.address();
        let response = aggregator::http::handlers::service::add_payload::add_payload(
            state.clone(),
            invalid_operator_payload,
            avs_client.service_manager_contract_address,
        )
        .await
        .unwrap_err();
        assert!(format!("{response:?}").contains("Operator is not registered"));
    }

    // Invalid signature
    {
        let mut invalid_signature_payload = signed_payload.clone();

        let payload_hash = eip191_hash_message(keccak256(signed_payload.data));

        let signature = invalid_signer.sign_hash_sync(&payload_hash).unwrap().into();

        invalid_signature_payload.signature = signature;
        let response = aggregator::http::handlers::service::add_payload::add_payload(
            state,
            invalid_signature_payload,
            avs_client.service_manager_contract_address,
        )
        .await
        .unwrap_err();
        assert!(format!("{response:?}").contains("Operator signature does not match"));
    }
}
