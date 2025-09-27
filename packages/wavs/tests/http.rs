#![cfg(feature = "dev")]
use axum::{
    body::Body,
    http::{Method, Request},
};
use tower::Service;
use utils::{
    config::{AnyChainConfig, CosmosChainConfig, EvmChainConfig},
    test_utils::{address::rand_address_evm, mock_engine::COMPONENT_SQUARE_BYTES},
};
use wavs::config::Config;
mod wavs_systems;
use wavs_systems::{
    http::{map_response, TestHttpApp},
    mock_trigger_manager::mock_evm_event_trigger,
};
use wavs_types::{
    ChainKey, Component, ComponentDigest, ComponentSource, SignatureKind, UploadComponentResponse,
};

#[test]
fn http_not_found() {
    let app = TestHttpApp::new();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/does_not_exist")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 404);
}

#[test]
fn http_config() {
    let app = TestHttpApp::new();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/config")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());

    let config: Config = app.ctx.rt.block_on(map_response(response));

    assert_eq!(config.port, app.inner.config.port);
}

#[test]
fn http_upload_component() {
    let digest = ComponentDigest::hash(COMPONENT_SQUARE_BYTES);

    let app = TestHttpApp::new();

    let body = Body::from(COMPONENT_SQUARE_BYTES);

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/dev/components")
        .body(body)
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());

    let response: UploadComponentResponse = app.ctx.rt.block_on(map_response(response));

    assert_eq!(response.digest, digest);
}

#[test]
fn http_save_service() {
    let app = TestHttpApp::new();

    let service = wavs_types::Service::new_simple(
        Some("My amazing service".to_string()),
        mock_evm_event_trigger(),
        ComponentSource::Digest(ComponentDigest::hash([1, 2, 3])),
        wavs_types::Submit::Aggregator {
            url: "http://example.com/aggregator".to_string(),
            component: Box::new(Component::new(ComponentSource::Digest(
                ComponentDigest::hash([1, 2, 3]),
            ))),
            signature_kind: SignatureKind::evm_default(),
        },
        wavs_types::ServiceManager::Evm {
            chain: "evm:anvil".try_into().unwrap(),
            address: rand_address_evm(),
        },
    );

    let body = Body::from(serde_json::to_vec(&service).unwrap());

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/dev/services")
        .body(body)
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());

    let service_hash = service.hash().unwrap();
    // retrieving the wrong service id should fail even if it's a partial match
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/dev/services/{}",
            service_hash.to_string().split_off(5)
        ))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(!response.status().is_success());

    // now get the real one and ensure it's what we originally sent
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/dev/services/{service_hash}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());

    let response: wavs_types::Service = app.ctx.rt.block_on(map_response(response));

    assert_eq!(response, service);

    tracing::info!("Service: {} round-tripped!", response.id());
}

fn create_test_evm_chain_config() -> AnyChainConfig {
    AnyChainConfig::Evm(EvmChainConfig {
        chain_id: "1337".parse().unwrap(),
        ws_endpoint: Some("wss://localhost:8546".to_string()),
        http_endpoint: Some("http://localhost:8545".to_string()),
        faucet_endpoint: None,
        poll_interval_ms: Some(1000),
        channel_size: EvmChainConfig::default_channel_size(),
    })
}

fn create_test_cosmos_chain_config() -> AnyChainConfig {
    AnyChainConfig::Cosmos(CosmosChainConfig {
        chain_id: "test-cosmos-1".parse().unwrap(),
        bech32_prefix: "cosmos".to_string(),
        rpc_endpoint: Some("http://localhost:26657".to_string()),
        grpc_endpoint: Some("http://localhost:9090".to_string()),
        gas_price: 0.025,
        gas_denom: "uatom".to_string(),
        faucet_endpoint: None,
    })
}

#[test]
fn test_add_chain_evm_success() {
    let app = TestHttpApp::new();

    let chain_config = create_test_evm_chain_config();
    let chain: ChainKey = format!("evm:{}", chain_config.chain_id().as_str())
        .parse()
        .unwrap();

    let request_body = serde_json::json!({
        "chain": chain,
        "config": chain_config
    });

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/chains")
        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 200);
}

#[test]
fn test_add_chain_cosmos_success() {
    let app = TestHttpApp::new();
    let chain_config = create_test_cosmos_chain_config();
    let chain: ChainKey = format!("cosmos:{}", chain_config.chain_id().as_str())
        .parse()
        .unwrap();

    let request_body = serde_json::json!({
        "chain": chain,
        "config": chain_config
    });

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/chains")
        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 200);
}

#[test]
fn test_add_chain_invalid_json() {
    let app = TestHttpApp::new();

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/chains")
        .body(Body::from("invalid json"))
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 400);
}

#[test]
fn test_add_chain_invalid_config() {
    let app = TestHttpApp::new();

    let request_body = serde_json::json!({
        "chain": "test-chain",
        "chain_config": {
            "invalid_type": {
                "chain_id": "1337"
            }
        }
    });

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/chains")
        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 422);
}

#[test]
fn test_add_chain_prevents_duplicates() {
    let app = TestHttpApp::new();
    let chain_config = create_test_evm_chain_config();
    let chain: ChainKey = format!("evm:{}", chain_config.chain_id().as_str())
        .parse()
        .unwrap();

    // add chain first time
    let add_request1 = serde_json::json!({
        "chain": chain,
        "config": chain_config
    });

    let req1 = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/chains")
        .body(Body::from(serde_json::to_vec(&add_request1).unwrap()))
        .unwrap();

    let response1 = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req1).await.unwrap() }
    });

    assert_eq!(
        response1.status(),
        200,
        "First chain addition should succeed"
    );

    // Try to add same chain again - should fail
    let add_request2 = serde_json::json!({
        "chain": chain,
        "config": chain_config
    });

    let req2 = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/chains")
        .body(Body::from(serde_json::to_vec(&add_request2).unwrap()))
        .unwrap();

    let response2 = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req2).await.unwrap() }
    });

    assert_eq!(
        response2.status(),
        500,
        "Duplicate chain addition should fail with 500"
    );
}

#[test]
fn body_size_limit() {
    wavs::init_tracing_tests();
    let app = TestHttpApp::new();

    // 14MB body succeeds (under default 15MB limit)
    let body_14mb = vec![0u8; 14 * 1024 * 1024];
    let req = Request::builder()
        .method(Method::POST)
        .uri("/dev/components")
        .body(Body::from(body_14mb))
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_ne!(
        response.status(),
        413,
        "14MB body should not be rejected (under 15MB limit)"
    );

    // 16MB body fails with 413
    let body_16mb = vec![0u8; 16 * 1024 * 1024];
    let req = Request::builder()
        .method(Method::POST)
        .uri("/dev/components")
        .body(Body::from(body_16mb))
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(
        response.status(),
        413,
        "16MB body should be rejected with 413 Payload Too Large"
    );
}
