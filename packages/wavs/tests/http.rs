use axum::{
    body::Body,
    http::{Method, Request},
};
use tower::Service;
use wavs::{
    config::Config,
    test_utils::{
        address::rand_address_evm,
        http::{map_response, TestHttpApp},
        mock_submissions::mock_eigen_submit,
        mock_trigger_manager::mock_evm_event_trigger,
    },
};
use wavs_types::{ComponentSource, Digest, ServiceID, UploadComponentResponse};

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
    let bytes = include_bytes!("../../../examples/build/components/square.wasm").to_vec();
    let digest = Digest::new(&bytes);

    let app = TestHttpApp::new();

    let body = Body::from(bytes);

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
        mock_eigen_submit(),
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

    // retrieving the wrong service id should fail even if it's a partial match
    let req = Request::builder()
        .method(Method::GET)
        .uri("/service/service-10")
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
        .uri("/service/service-1")
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
