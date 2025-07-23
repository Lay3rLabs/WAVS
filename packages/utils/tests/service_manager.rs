use alloy_node_bindings::Anvil;
use utils::{
    evm_client::EvmSigningClient,
    init_tracing_tests,
    test_utils::{
        address::rand_address_evm,
        middleware::{AvsOperator, MiddlewareServiceManagerConfig},
        mock_service_manager::MockServiceManager,
    },
};

#[tokio::test]
async fn service_manager_deployment() {
    init_tracing_tests();

    let anvil = Anvil::new().spawn();

    let client = EvmSigningClient::new_anvil(&anvil.endpoint())
        .await
        .unwrap();

    let avs_operator = AvsOperator::new(rand_address_evm(), rand_address_evm());
    let config = MiddlewareServiceManagerConfig::new(&[avs_operator], 1);

    let service_manager = MockServiceManager::deploy_middleware(config, client)
        .await
        .unwrap();

    assert!(
        !service_manager.address.is_zero(),
        "Service Manager implementation address should not be zero"
    );
}
