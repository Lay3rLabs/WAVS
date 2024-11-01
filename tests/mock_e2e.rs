use helpers::{http::TestHttpApp, service::MockServiceBuilder};
use tower::Service;
use wasmatic::{dispatcher::MockDispatcherBuilder, http::handlers::service::add::RegisterAppRequest};
use axum::http::{Method, Request};

mod helpers;

// this is like the real e2e but with only mocks
// does not test throughput with real pipelinning
// intended more to confirm API and logic is working as expected
#[tokio::test]
async fn mock_e2e() {

    let mut dispatcher = MockDispatcherBuilder::new();

    let mut app = TestHttpApp::new().await;

    create_service(&mut app, "test-service").await;



    // let tx_resp = task_queue
    // .submit_task("squaring 3", serde_json::json!({ "x": 3 }))
    // .await
    // .unwrap();
}

async fn create_service(app: &mut TestHttpApp, name: impl ToString) {
    let body = serde_json::to_string(&RegisterAppRequest {
        app: MockServiceBuilder::new(name).build(),
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
}