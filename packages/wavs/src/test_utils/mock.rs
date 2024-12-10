use std::sync::Arc;

use super::http::{map_response, TestHttpApp};
use crate::{
    apis::{
        dispatcher::{DispatchManager, Permissions, Submit},
        engine::EngineError,
        ID,
    },
    context::AppContext,
    dispatcher::Dispatcher,
    engine::{
        mock::{Function, MockEngine},
        runner::{EngineRunner, SingleEngineRunner},
    },
    http::{
        handlers::service::{
            add::{AddServiceRequest, ServiceRequest},
            delete::DeleteServices,
            list::ListServicesResponse,
            test::{TestAppRequest, TestAppResponse},
        },
        types::TriggerRequest,
    },
    submission::mock::MockSubmission,
    triggers::mock::MockTriggerManagerChannel,
    Digest,
};
use axum::{
    body::Body,
    http::{Method, Request},
};
use layer_climb::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tower::Service;

pub struct MockE2ETestRunner {
    pub ctx: AppContext,
    pub dispatcher:
        Arc<Dispatcher<MockTriggerManagerChannel, SingleEngineRunner<MockEngine>, MockSubmission>>,
    pub http_app: TestHttpApp,
}

impl MockE2ETestRunner {
    pub fn new(ctx: AppContext) -> Arc<Self> {
        // create our dispatcher
        let trigger_manager = MockTriggerManagerChannel::new(10);
        let engine = SingleEngineRunner::new(MockEngine::new());
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

    pub async fn list_services(&self) -> ListServicesResponse {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/app")
            .body(Body::empty())
            .unwrap();

        let response = self
            .http_app
            .clone()
            .http_router()
            .await
            .call(req)
            .await
            .unwrap();

        map_response::<ListServicesResponse>(response).await
    }

    pub async fn create_service_simple(
        &self,
        service_id: ID,
        digest: Digest,
        task_queue_address: &Address,
        task_queue_erc1271: &Address,
        function: impl Function,
    ) {
        self.create_service(
            service_id,
            digest,
            task_queue_address,
            task_queue_erc1271,
            Permissions::default(),
            Vec::new(),
            function,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_service(
        &self,
        service_id: ID,
        digest: Digest,
        task_queue_address: &Address,
        task_queue_erc1271: &Address,
        permissions: Permissions,
        envs: Vec<(String, String)>,
        function: impl Function,
    ) {
        // "upload" the component
        // not going through http for this because we don't have raw bytes, digest is fake
        self.dispatcher.engine.engine().register(&digest, function);

        // but we can create a service via http router
        let body = serde_json::to_string(&AddServiceRequest {
            service: ServiceRequest {
                trigger: TriggerRequest::eth_queue(
                    task_queue_address.clone(),
                    task_queue_erc1271.clone(),
                ),
                id: service_id,
                digest: digest.into(),
                permissions,
                envs,
                testable: None,
                submit: Submit::eth_aggregator_tx(),
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

    pub async fn delete_services(&self, service_ids: Vec<ID>) {
        let body = serde_json::to_string(&DeleteServices { service_ids }).unwrap();

        let req = Request::builder()
            .method(Method::DELETE)
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

    pub async fn test_service<D: DeserializeOwned>(
        &self,
        service_id: ID,
        input: impl Serialize,
    ) -> D {
        let body = serde_json::to_string(&TestAppRequest {
            name: service_id.to_string(),
            input: Some(serde_json::to_value(input).unwrap()),
        })
        .unwrap();

        let req = Request::builder()
            .method(Method::POST)
            .header("Content-Type", "application/json")
            .uri("/test")
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

        let res = map_response::<TestAppResponse>(response).await;

        serde_json::from_value(res.output).unwrap()
    }

    pub fn teardown(&self) {
        // Your teardown code here
    }
}

// taken from dispatcher unit test
pub struct BigSquare;

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct SquareIn {
    pub x: u64,
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]

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
