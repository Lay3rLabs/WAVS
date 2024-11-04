use axum::{
    body::Body,
    http::{Method, Request},
};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use tower::Service;
use wasmatic::{
    config::Config,
    http::{
        handlers::service::{
            add::{RegisterAppRequest, RegisterAppResponse},
            delete::DeleteApps,
            list::ListAppsResponse,
            test::{TestAppRequest, TestAppResponse},
            upload::UploadServiceResponse,
        },
        types::app::Status,
    },
    test_utils::{http::TestHttpApp, service::MockServiceBuilder},
    Digest,
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
async fn http_add_service() {
    let mut app = TestHttpApp::new().await;

    let body = serde_json::to_string(&RegisterAppRequest {
        app: MockServiceBuilder::new("mock-service").build(),
        wasm_url: None,
    })
    .unwrap();

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/app")
        .body(body)
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert!(response.status().is_success());

    let response: RegisterAppResponse = map_response(response).await;

    assert_eq!(response.name, "mock-service");
    assert_eq!(response.status, Status::Active);
}

#[tokio::test]
async fn http_delete_service() {
    let mut app = TestHttpApp::new().await;

    let body = serde_json::to_string(&DeleteApps {
        apps: vec!["mock-service".to_string()],
    })
    .unwrap();

    let req = Request::builder()
        .method(Method::DELETE)
        .header("Content-Type", "application/json")
        .uri("/app")
        .body(body)
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert!(response.status().is_success());
}

#[tokio::test]
async fn http_list_services() {
    let mut app = TestHttpApp::new().await;

    let req = Request::builder()
        .method(Method::GET)
        .header("Content-Type", "application/json")
        .uri("/app")
        .body(Body::empty())
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert!(response.status().is_success());

    let response: ListAppsResponse = map_response(response).await;

    assert_eq!(
        response
            .apps
            .into_iter()
            .map(|app| app.name)
            .collect::<Vec<String>>(),
        vec!["mock-service-1", "mock-service-2"]
    );
}

#[tokio::test]
async fn http_test_service() {
    let mut app = TestHttpApp::new().await;

    let body = serde_json::to_string(&TestAppRequest {
        name: "mock-service".to_string(),
        input: Some(serde_json::json!({
            "x": 3
        })),
    })
    .unwrap();

    let req = Request::builder()
        .method(Method::POST)
        .header("Content-Type", "application/json")
        .uri("/test")
        .body(body)
        .unwrap();

    let response = app.http_router().await.call(req).await.unwrap();

    assert!(response.status().is_success());

    let response: TestAppResponse = map_response(response).await;

    assert_eq!(response.output, serde_json::json!({ "y": 9.0 }));
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

    assert_eq!(response.digest, digest);
}

async fn map_response<T: DeserializeOwned>(response: axum::http::Response<Body>) -> T {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}
