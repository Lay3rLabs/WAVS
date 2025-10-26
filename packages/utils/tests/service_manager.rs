use utils::{
    evm_client::EvmSigningClient,
    init_tracing_tests,
    test_utils::{
        address::rand_address_evm,
        anvil::safe_spawn_anvil,
        middleware::evm::{
            AvsOperator, EvmMiddleware, EvmMiddlewareType, MiddlewareServiceManagerConfig,
        },
        mock_service_manager::MockEvmServiceManager,
    },
};

#[tokio::test]
async fn service_manager_deployment() {
    init_tracing_tests();

    let anvil = safe_spawn_anvil();

    let client = EvmSigningClient::new_anvil(&anvil.endpoint())
        .await
        .unwrap();

    let middleware = EvmMiddleware::new(EvmMiddlewareType::Eigenlayer).unwrap();

    // deploy
    let service_manager = MockEvmServiceManager::new(middleware, client)
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
