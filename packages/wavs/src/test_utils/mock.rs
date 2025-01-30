use std::sync::Arc;

use super::http::{map_response, TestHttpApp};
use crate::{
    apis::{dispatcher::DispatchManager, engine::EngineError},
    dispatcher::Dispatcher,
    engine::{
        mock::{Function, MockEngine},
        runner::{EngineRunner, SingleEngineRunner},
    },
    submission::mock::MockSubmission,
    test_utils::address::rand_address_eth,
    triggers::mock::{mock_eth_event_trigger, MockTriggerManagerChannel},
    AppContext,
};
use axum::{
    body::Body,
    http::{Method, Request},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tower::Service as _;
use utils::{
    digest::Digest,
    types::{
        AddServiceRequest, DeleteServicesRequest, ListServicesResponse, Service, Submit,
        TestAppRequest, TestAppResponse, TriggerData,
    },
    ServiceID,
};

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
        service_id: ServiceID,
        digest: Digest,
        function: impl Function,
    ) {
        self.create_service(service_id, digest, function).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_service(
        &self,
        service_id: ServiceID,
        digest: Digest,
        function: impl Function,
    ) {
        // "upload" the component
        // not going through http for this because we don't have raw bytes, digest is fake
        self.dispatcher.engine.engine().register(&digest, function);

        // but we can create a service via http router
        let trigger = mock_eth_event_trigger();

        let submit = Submit::eigen_contract("eth".try_into().unwrap(), rand_address_eth(), None);

        let service =
            Service::new_simple(service_id, "mock-service", trigger, digest, submit, None);

        let body = serde_json::to_string(&AddServiceRequest { service }).unwrap();

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

    pub async fn delete_services(&self, service_ids: Vec<ServiceID>) {
        let body = serde_json::to_string(&DeleteServicesRequest { service_ids }).unwrap();

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
        service_id: ServiceID,
        input: impl Serialize,
    ) -> D {
        let body = serde_json::to_string(&TestAppRequest {
            name: service_id.to_string(),
            input: TriggerData::Raw(serde_json::to_vec(&input).unwrap()),
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
    fn execute(&self, request: Vec<u8>) -> Result<Vec<u8>, EngineError> {
        let SquareIn { x } = serde_json::from_slice(&request).unwrap();
        let output = SquareOut { y: x * x };
        Ok(serde_json::to_vec(&output).unwrap())
    }
}
