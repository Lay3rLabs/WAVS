mod helpers;
use axum::{
    body::Body,
    http::{Method, Request},
};
use helpers::http::TestHttpApp;
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use tower::Service;
use wasmatic::config::Config;

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

    assert_eq!(response.status(), 200);

    let config: Config = map_response(response).await;

    assert_eq!(config.port, app.inner.config.port);
}

async fn map_response<T: DeserializeOwned>(response: axum::http::Response<Body>) -> T {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}
