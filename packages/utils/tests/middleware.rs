use utils::{
    init_tracing_tests,
    test_utils::middleware::{MiddlewareInstance, MiddlewareType},
};

#[tokio::test]
async fn middleware_instantiation() {
    init_tracing_tests();

    let middleware = MiddlewareInstance::new(MiddlewareType::Eigenlayer)
        .await
        .unwrap();

    tracing::info!("Middleware container ID: {}", middleware.container_id());

    assert!(
        !middleware.container_id().is_empty(),
        "Middleware container ID should not be empty"
    );
}
