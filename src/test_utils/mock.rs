use std::sync::Arc;

use super::http::TestHttpApp;
use crate::{
    apis::{
        dispatcher::DispatchManager,
        engine::EngineError,
        trigger::{TriggerAction, TriggerData, TriggerResult},
        IDError, Trigger, ID,
    },
    context::AppContext,
    dispatcher::Dispatcher,
    engine::mock::{Function, MockEngine},
    http::{handlers::service::add::RegisterAppRequest, types::app::App},
    submission::mock::MockSubmission,
    triggers::mock::MockTriggerManagerChannel,
    Digest,
};
use axum::http::{Method, Request};
use lavs_apis::id::TaskId;
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use tower::Service;

pub struct MockE2ETestRunner {
    pub ctx: AppContext,
    pub dispatcher: Arc<Dispatcher<MockTriggerManagerChannel, MockEngine, MockSubmission>>,
    pub http_app: TestHttpApp,
}

impl MockE2ETestRunner {
    pub fn new() -> Arc<Self> {
        // create our app context
        let ctx = AppContext::new();

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
        let http_app = ctx.rt.block_on({
            let dispatcher = dispatcher.clone();
            async move { TestHttpApp::new_with_dispatcher(dispatcher).await }
        });

        Arc::new(Self {
            ctx,
            dispatcher,
            http_app,
        })
    }

    pub async fn create_service(
        &self,
        service_id: ID,
        digest: Digest,
        task_queue_address: &Address,
        function: impl Function,
    ) {
        // "upload" the component
        // not going through http for this because we don't have raw bytes, digest is fake
        self.dispatcher.engine.register(&digest, function);

        // but we can create a service via http router
        let body = serde_json::to_string(&RegisterAppRequest {
            app: App {
                trigger: Trigger::queue(&task_queue_address.to_string(), 5),
                name: service_id.to_string(),
                status: None,
                digest,
                permissions: crate::http::types::app::Permissions {},
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

        let response = self
            .http_app
            .clone()
            .http_router()
            .await
            .call(req)
            .await
            .unwrap();

        assert!(response.status().is_success());
    }

    pub async fn send_trigger(
        &self,
        service_id: impl TryInto<ID, Error = IDError>,
        workflow_id: impl TryInto<ID, Error = IDError>,
        task_queue_addr: &Address,
        data: &impl Serialize,
    ) {
        self.dispatcher
            .triggers
            .sender
            .send(TriggerAction {
                trigger: TriggerData::queue(
                    service_id,
                    workflow_id,
                    &task_queue_addr.to_string(),
                    5,
                )
                .unwrap(),
                result: TriggerResult::queue(
                    TaskId::new(1),
                    serde_json::to_string(data).unwrap().as_bytes(),
                ),
            })
            .await
            .unwrap();
    }

    pub fn teardown(&self) {
        // Your teardown code here
    }
}

// taken from dispatcher unit test
pub struct BigSquare;

#[derive(Deserialize, Serialize)]
pub struct SquareIn {
    pub x: u64,
}

#[derive(Deserialize, Serialize)]
pub struct SquareOut {
    pub y: u64,
}

impl Function for BigSquare {
    fn execute(&self, request: Vec<u8>, _timestamp: u64) -> Result<Vec<u8>, EngineError> {
        let SquareIn { x } = serde_json::from_slice(&request).unwrap();
        let output = SquareOut { y: x * x };
        Ok(serde_json::to_vec(&output).unwrap())
    }
}
