use axum::{
    body::Body,
    http::{Method, Request},
};
use tower::Service;
use utils::{digest::Digest, types::UploadServiceResponse};
use wavs::{
    config::Config,
    test_utils::http::{map_response, TestHttpApp},
};

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
async fn http_upload_service() {
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

    let response: UploadServiceResponse = map_response(response).await;

    assert_eq!(response.digest, digest.into());
}
