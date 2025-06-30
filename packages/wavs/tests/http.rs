use axum::{
    body::Body,
    http::{Method, Request},
};
use tower::Service;
use utils::{
    config::{AnyChainConfig, CosmosChainConfig, EvmChainConfig},
    test_utils::{address::rand_address_evm, mock_engine::COMPONENT_SQUARE},
};
use wavs::config::Config;
mod wavs_systems;
use wavs_systems::{
    http::{map_response, TestHttpApp},
    mock_trigger_manager::mock_evm_event_trigger,
};
use wavs_types::{ChainName, ComponentSource, Digest, ServiceID, UploadComponentResponse};

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
    let digest = Digest::new(COMPONENT_SQUARE);

    let app = TestHttpApp::new();

    let body = Body::from(COMPONENT_SQUARE);

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/upload")
        .body(body)
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());

    let response: UploadComponentResponse = app.ctx.rt.block_on(map_response(response));

    assert_eq!(response.digest, digest.into());
}

#[test]
fn http_save_service() {
    let app = TestHttpApp::new();

    let service = wavs_types::Service::new_simple(
        ServiceID::new("service-1").unwrap(),
        Some("My amazing service".to_string()),
        mock_evm_event_trigger(),
        ComponentSource::Digest(Digest::new(&[1, 2, 3])),
        wavs_types::Submit::Aggregator {
            url: "http://example.com/aggregator".to_string(),
        },
        wavs_types::ServiceManager::Evm {
            chain_name: "evm".try_into().unwrap(),
            address: rand_address_evm(),
        },
    );

    let body = Body::from(serde_json::to_vec(&service).unwrap());

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/save-service")
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
            "/service-by-hash/{}",
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
        .uri(format!("/service-by-hash/{service_hash}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());

    let response: wavs_types::Service = app.ctx.rt.block_on(map_response(response));

    assert_eq!(response, service);

    tracing::info!("Service: {} round-tripped!", response.id);
}

fn create_test_evm_chain_config() -> AnyChainConfig {
    AnyChainConfig::Evm(EvmChainConfig {
        chain_id: "1337".to_string(),
        ws_endpoint: Some("wss://localhost:8546".to_string()),
        http_endpoint: Some("http://localhost:8545".to_string()),
        faucet_endpoint: None,
        poll_interval_ms: Some(1000),
    })
}

fn create_test_cosmos_chain_config() -> AnyChainConfig {
    AnyChainConfig::Cosmos(CosmosChainConfig {
        chain_id: "test-cosmos-1".to_string(),
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
    let chain_name: ChainName = "test-evm".try_into().unwrap();
    let chain_config = create_test_evm_chain_config();

    let request_body = serde_json::json!({
        "chain_name": chain_name,
        "chain_config": chain_config
    });

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/add-chain")
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
    let chain_name: ChainName = "test-cosmos".try_into().unwrap();
    let chain_config = create_test_cosmos_chain_config();

    let request_body = serde_json::json!({
        "chain_name": chain_name,
        "chain_config": chain_config
    });

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/add-chain")
        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 200);
}

#[test]
fn test_add_chain_duplicate_name() {
    let app = TestHttpApp::new();
    let chain_name: ChainName = "duplicate-chain".try_into().unwrap();
    let chain_config = create_test_evm_chain_config();

    let request_body = serde_json::json!({
        "chain_name": chain_name,
        "chain_config": chain_config
    });

    let body = Body::from(serde_json::to_vec(&request_body).unwrap());

    let req1 = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/add-chain")
        .body(body)
        .unwrap();

    let response1 = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req1).await.unwrap() }
    });

    assert_eq!(response1.status(), 200);

    let request_body2 = serde_json::json!({
        "chain_name": chain_name,
        "chain_config": chain_config
    });

    let req2 = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/add-chain")
        .body(Body::from(serde_json::to_vec(&request_body2).unwrap()))
        .unwrap();

    let response2 = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req2).await.unwrap() }
    });

    assert_eq!(response2.status(), 500);
}

#[test]
fn test_add_chain_invalid_json() {
    let app = TestHttpApp::new();

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/add-chain")
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
        "chain_name": "test-chain",
        "chain_config": {
            "invalid_type": {
                "chain_id": "1337"
            }
        }
    });

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/add-chain")
        .body(Body::from(serde_json::to_vec(&request_body).unwrap()))
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 422);
}
