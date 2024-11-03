use std::sync::Arc;

use axum::http::{Method, Request};
use helpers::{chain::MOCK_TASK_QUEUE_ADDRESS, http::TestHttpApp};
use lavs_apis::id::TaskId;
use serde::{Deserialize, Serialize};
use tower::Service;
use wasmatic::{
    apis::{
        dispatcher::DispatchManager,
        engine::EngineError,
        trigger::{TriggerAction, TriggerData, TriggerResult},
        Trigger, ID,
    },
    context::AppContext,
    dispatcher::Dispatcher,
    engine::mock::{Function, MockEngine},
    http::{handlers::service::add::RegisterAppRequest, types::app::App},
    submission::mock::MockSubmission,
    triggers::mock::MockTriggerManagerChannel,
    Digest,
};

mod helpers;

// this is like the real e2e but with only mocks
// does not test throughput with real pipelinning
// intended more to confirm API and logic is working as expected
#[test]
fn mock_e2e() {
    let ctx = AppContext::new();

    let workflow_id = ID::new("default").unwrap();
    let service_id = ID::new("test-service").unwrap();

    // create our dispatcher
    let trigger_manager = MockTriggerManagerChannel::new(10);
    let engine = MockEngine::new();
    let submission = MockSubmission::new();
    let storage_path = tempfile::NamedTempFile::new().unwrap();

    let dispatcher =
        Arc::new(Dispatcher::new(trigger_manager, engine, submission, storage_path).unwrap());

    // start up the dispatcher in its own thread, before creating any data (similar to how we do it in main)
    std::thread::spawn({
        let ctx = ctx.clone();
        let dispatcher = dispatcher.clone();
        move || {
            dispatcher.start(ctx).unwrap();
        }
    });

    // start up our "http server" in its own thread, before creating any data (similar to how we do it in main)
    let app = ctx.rt.block_on({
        let dispatcher = dispatcher.clone();
        async move { TestHttpApp::new_with_dispatcher(dispatcher).await }
    });

    // "upload" a component that squares a number
    // not going through http for this because we don't have raw bytes, digest is fake
    let digest = Digest::new(b"wasm1");
    dispatcher.engine.register(&digest, BigSquare);

    // but we can create a service via http
    ctx.rt.spawn({
        let mut app = app.clone();
        let service_id = service_id.clone();
        async move {
            create_service(&mut app, &service_id, digest).await;
        }
    });

    // now pretend like we're reading some triggers off the chain
    // this spawned into the async runtime, so it's sortof like the real TriggerManager
    ctx.rt.spawn({
        let dispatcher = dispatcher.clone();
        let service_id = service_id.clone();
        async move {
            dispatcher
                .triggers
                .sender
                .send(TriggerAction {
                    trigger: TriggerData::queue(&service_id, &workflow_id, "layer1taskqueue", 5)
                        .unwrap(),
                    result: TriggerResult::queue(TaskId::new(1), br#"{"x":3}"#),
                })
                .await
                .unwrap();

            dispatcher
                .triggers
                .sender
                .send(TriggerAction {
                    trigger: TriggerData::queue(&service_id, &workflow_id, "layer1taskqueue", 5)
                        .unwrap(),
                    result: TriggerResult::queue(TaskId::new(2), br#"{"x":21}"#),
                })
                .await
                .unwrap();
        }
    });

    // block and wait for triggers to go through the whole flow
    dispatcher.submission.wait_for_messages(2).unwrap();

    // check the results
    let results: Vec<serde_json::Value> = dispatcher
        .submission
        .received()
        .iter()
        .map(|msg| serde_json::from_slice(&msg.wasm_result).unwrap())
        .collect();

    tracing::info!("results: {:?}", results);

    assert_eq!(
        results,
        vec![serde_json::json!({"y": 9}), serde_json::json!({"y": 441})]
    );
}

async fn create_service(app: &mut TestHttpApp, name: impl ToString, digest: Digest) {
    let body = serde_json::to_string(&RegisterAppRequest {
        app: App {
            trigger: Trigger::queue(&MOCK_TASK_QUEUE_ADDRESS.to_string(), 5),
            name: name.to_string(),
            status: None,
            digest,
            permissions: wasmatic::http::types::app::Permissions {},
            envs: Vec::new(),
            testable: None,
        },
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

// taken from dispatcher unit test
pub struct BigSquare;

#[derive(Deserialize, Serialize)]
struct SquareIn {
    pub x: u64,
}

#[derive(Deserialize, Serialize)]
struct SquareOut {
    pub y: u64,
}

impl Function for BigSquare {
    fn execute(&self, request: Vec<u8>, _timestamp: u64) -> Result<Vec<u8>, EngineError> {
        let SquareIn { x } = serde_json::from_slice(&request).unwrap();
        let output = SquareOut { y: x * x };
        Ok(serde_json::to_vec(&output).unwrap())
    }
}
