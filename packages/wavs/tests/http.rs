#![cfg(feature = "dev")]
use axum::{
    body::Body,
    http::{Method, Request},
};
use tower::Service;
use utils::test_utils::{address::rand_address_evm, mock_engine::COMPONENT_SQUARE_BYTES};
use wavs::config::Config;
mod wavs_systems;
use wavs::health::HealthStatus;
use wavs_systems::{
    http::{map_response, TestHttpApp},
    mock_trigger_manager::mock_evm_event_trigger,
};
use wavs_types::{
    AnyChainConfig, ChainKey, Component, ComponentDigest, ComponentSource, CosmosChainConfig,
    EvmChainConfig, SignatureKind, UploadComponentResponse,
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
        .uri("/dev/config")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());

    let config: Config = app.ctx.rt.block_on(map_response(response));

    assert_eq!(config.port, app.inner.config.port);

    // Verify credentials are redacted
    if let Some(cred) = &config.signing_mnemonic {
        assert_eq!(
            cred.as_str(),
            "redacted",
            "signing_mnemonic should be redacted"
        );
    }
    if let Some(cred) = &config.aggregator_cosmos_credential {
        assert_eq!(
            cred.as_str(),
            "redacted",
            "aggregator_cosmos_credential should be redacted"
        );
    }
    if let Some(cred) = &config.aggregator_evm_credential {
        assert_eq!(
            cred.as_str(),
            "redacted",
            "aggregator_evm_credential should be redacted"
        );
    }
    if let Some(cred) = &config.bearer_token {
        assert_eq!(cred.as_str(), "redacted", "bearer_token should be redacted");
    }
}

#[test]
fn http_health() {
    let app = TestHttpApp::new();

    let req = Request::builder()
        .method(Method::GET)
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 200);

    let health_status: HealthStatus = app.ctx.rt.block_on(map_response(response));

    assert!(health_status.timestamp > 0);
    assert!(health_status.is_healthy());
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
        ws_endpoints: vec!["wss://localhost:8546".to_string()],
        http_endpoint: Some("http://localhost:8545".to_string()),
        faucet_endpoint: None,
        ws_priority_endpoint_index: None,
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
        .uri("/dev/chains")
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
        .uri("/dev/chains")
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
        .uri("/dev/chains")
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
        .uri("/dev/chains")
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
        .uri("/dev/chains")
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
        .uri("/dev/chains")
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
fn http_logs_polling() {
    use wavs::http::handlers::logs::LogsResponse;
    use wavs::log_buffer::LogEntry;

    let app = TestHttpApp::new();

    // Push entries directly into the buffer (simulates InMemoryLogLayer capturing events)
    for i in 0..5u64 {
        app.log_buffer.push(LogEntry {
            id: 0, // assigned by push()
            timestamp_ms: 1_000 + i,
            level: if i % 2 == 0 {
                "INFO".to_string()
            } else {
                "WARN".to_string()
            },
            target: "wavs::test".to_string(),
            fields: format!("message=\"entry {i}\""),
        });
    }

    // Fetch all logs
    let req = Request::builder()
        .method(Method::GET)
        .uri("/dev/logs")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());
    let resp: LogsResponse = app.ctx.rt.block_on(map_response(response));
    assert_eq!(resp.entries.len(), 5);
    assert_eq!(resp.next_id, 5);

    // Fetch only WARN entries
    let req = Request::builder()
        .method(Method::GET)
        .uri("/dev/logs?level=warn")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());
    let resp: LogsResponse = app.ctx.rt.block_on(map_response(response));
    // entries 1, 3 are WARN
    assert_eq!(resp.entries.len(), 2);
    assert!(resp.entries.iter().all(|e| e.level == "WARN"));

    // Incremental poll: fetch only entries with id >= 3
    let req = Request::builder()
        .method(Method::GET)
        .uri("/dev/logs?since_id=3")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());
    let resp: LogsResponse = app.ctx.rt.block_on(map_response(response));
    assert_eq!(resp.entries.len(), 2); // entries 3 and 4
    assert_eq!(resp.entries[0].id, 3);
}

#[test]
fn http_logs_invalid_level_returns_400() {
    let app = TestHttpApp::new();

    for uri in ["/dev/logs?level=verbose", "/dev/logs/stream?level=verbose"] {
        let req = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().ctx.rt.block_on({
            let mut app = app.clone();
            async move { app.http_router().await.call(req).await.unwrap() }
        });

        assert_eq!(
            response.status(),
            400,
            "expected 400 for invalid level in {uri}"
        );
    }
}

#[test]
fn http_logs_zero_limit_clamped() {
    use wavs::http::handlers::logs::LogsResponse;
    use wavs::log_buffer::LogEntry;

    let app = TestHttpApp::new();

    app.log_buffer.push(LogEntry {
        id: 0, // assigned by push()
        timestamp_ms: 1000,
        level: "INFO".to_string(),
        target: "wavs::test".to_string(),
        fields: "message=\"hello\"".to_string(),
    });

    // limit=0 should be clamped to 1, not return an empty list
    let req = Request::builder()
        .method(Method::GET)
        .uri("/dev/logs?limit=0")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert!(response.status().is_success());
    let resp: LogsResponse = app.ctx.rt.block_on(map_response(response));
    assert_eq!(resp.entries.len(), 1, "limit=0 should be clamped to 1");
}

/// Read exactly `n` SSE events from a streaming response body, with a per-frame timeout.
/// Each SSE event is delimited by a blank line (`\n\n`).
async fn read_sse_events(body: axum::body::Body, n: usize) -> Vec<wavs::log_buffer::LogEntry> {
    use http_body_util::BodyExt;

    let mut body = body;
    let mut raw = String::new();
    let mut events = Vec::new();

    while events.len() < n {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
            .await
            .expect("timeout reading SSE frame")
            .expect("body ended before expected frame count")
            .expect("frame error");

        if let Ok(data) = frame.into_data() {
            raw.push_str(&String::from_utf8_lossy(&data));
        }

        // Parse all complete SSE events accumulated so far
        while let Some(pos) = raw.find("\n\n") {
            let event_str = raw[..pos].to_string();
            raw = raw[pos + 2..].to_string();
            if let Some(json) = event_str.strip_prefix("data: ") {
                if let Ok(entry) = serde_json::from_str(json) {
                    events.push(entry);
                }
            }
        }
    }

    events
}

#[test]
fn http_logs_sse_replay_ordering() {
    use wavs::log_buffer::LogEntry;

    let app = TestHttpApp::new();

    // Push 4 entries before connecting: alternating INFO/WARN
    for i in 0..4u64 {
        app.log_buffer.push(LogEntry {
            id: 0, // assigned by push()
            timestamp_ms: 1000 + i,
            level: if i % 2 == 0 {
                "INFO".to_string()
            } else {
                "WARN".to_string()
            },
            target: "wavs::test".to_string(),
            fields: format!("message=\"sse-entry {i}\""),
        });
    }

    // Connect and receive all 4 replayed entries
    let req = Request::builder()
        .method(Method::GET)
        .uri("/dev/logs/stream")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 200);
    assert!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .starts_with("text/event-stream"),
        "expected SSE content-type"
    );

    let events = app
        .ctx
        .rt
        .block_on(read_sse_events(response.into_body(), 4));

    assert_eq!(events.len(), 4);
    // Entries arrive in id order
    for (i, e) in events.iter().enumerate() {
        assert_eq!(e.id, i as u64);
    }

    // Connect with level=warn filter — should replay only entries 1 and 3
    let req = Request::builder()
        .method(Method::GET)
        .uri("/dev/logs/stream?level=warn")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().ctx.rt.block_on({
        let mut app = app.clone();
        async move { app.http_router().await.call(req).await.unwrap() }
    });

    assert_eq!(response.status(), 200);
    let warn_events = app
        .ctx
        .rt
        .block_on(read_sse_events(response.into_body(), 2));

    assert_eq!(warn_events.len(), 2);
    assert!(warn_events.iter().all(|e| e.level == "WARN"));
    assert_eq!(warn_events[0].id, 1);
    assert_eq!(warn_events[1].id, 3);
}

#[test]
fn body_size_limit() {
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
