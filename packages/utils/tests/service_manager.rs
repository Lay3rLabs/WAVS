use alloy_node_bindings::Anvil;
use utils::{
    init_tracing_tests,
    test_utils::deploy_service_manager::{HexEncodedPrivateKey, ServiceManager, ServiceManagerConfig},
};

#[tokio::test]
async fn service_manager_deployment() {
    init_tracing_tests();

    let anvil = Anvil::new().spawn();

    let service_manager =
        ServiceManager::deploy(ServiceManagerConfig::default(), anvil.endpoint(), HexEncodedPrivateKey::new_anvil())
            .await
            .unwrap();

    assert!(
        !service_manager.address.is_zero(),
        "Service Manager implementation address should not be zero"
    );
}
