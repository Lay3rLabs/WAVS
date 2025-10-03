use utils::{
    evm_client::EvmSigningClient,
    init_tracing_tests,
    test_utils::{
        address::rand_address_evm,
        anvil::safe_spawn_anvil,
        middleware::{AvsOperator, MiddlewareInstance, MiddlewareServiceManagerConfig},
        mock_service_manager::MockServiceManager,
    },
};

#[tokio::test]
async fn service_manager_deployment() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil();

    let client = EvmSigningClient::new_anvil(&anvil.endpoint())
        .await
        .unwrap();

    let middleware_instance = MiddlewareInstance::new().await.unwrap();

    // deploy
    let service_manager = MockServiceManager::new(middleware_instance, client)
        .await
        .unwrap();

    // configure
    let avs_operator = AvsOperator::new(rand_address_evm(), rand_address_evm());
    let config = MiddlewareServiceManagerConfig::new(&[avs_operator], 1);
    service_manager.configure(&config).await.unwrap();

    // set service URI
    service_manager
        .set_service_uri("http://example.com".to_string())
        .await
        .unwrap();

    assert!(
        !service_manager.address().is_zero(),
        "Service Manager implementation address should not be zero"
    );

    tracing::info!("Service Manager deployed at: {}", service_manager.address());
}
