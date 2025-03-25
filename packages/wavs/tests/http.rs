use axum::{
    body::Body,
    http::{Method, Request},
};
use tower::Service;
use wavs::{
    config::Config,
    submission::mock::mock_eigen_submit,
    test_utils::http::{map_response, TestHttpApp},
    triggers::mock::mock_eth_event_trigger,
};
use wavs_types::{ComponentSource, Digest, ServiceID, UploadComponentResponse};

#[tokio::test]
async fn http_not_found() {
    let mut app = TestHttpApp::new().await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/does_not_exist")
        .body(Body::empty())
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn http_config() {
    let mut app = TestHttpApp::new().await;

    let req = Request::builder()
        .method(Method::GET)
        .uri("/config")
        .body(Body::empty())
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert!(response.status().is_success());

    let config: Config = map_response(response).await;

    assert_eq!(config.port, app.inner.config.port);
}

#[tokio::test]
async fn http_upload_component() {
    let bytes = vec![1, 2, 3];
    let digest = Digest::new(&bytes);

    let mut app = TestHttpApp::new().await;

    let body = Body::from(bytes);

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/upload")
        .body(body)
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert!(response.status().is_success());

    let response: UploadComponentResponse = map_response(response).await;

    assert_eq!(response.digest, digest.into());
}

#[tokio::test]
async fn http_save_service() {
    let mut app = TestHttpApp::new().await;

    let service = wavs_types::Service::new_simple(
        ServiceID::new("service-1").unwrap(),
        Some("My amazing service".to_string()),
        mock_eth_event_trigger(),
        ComponentSource::Digest(Digest::new(&[1, 2, 3])),
        mock_eigen_submit(),
        None,
    );

    let body = Body::from(serde_json::to_vec(&service).unwrap());

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/save-service")
        .body(body)
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert!(response.status().is_success());

    // retrieving the wrong service id should fail even if it's a partial match
    let req = Request::builder()
        .method(Method::GET)
        .uri("/service/service-10")
        .body(Body::empty())
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert!(!response.status().is_success());

    // now get the real one and ensure it's what we originally sent
    let req = Request::builder()
        .method(Method::GET)
        .uri("/service/service-1")
        .body(Body::empty())
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert!(response.status().is_success());

    let response: wavs_types::Service = map_response(response).await;

    assert_eq!(response, service);

    tracing::info!("Service: {} round-tripped!", response.id);
}
