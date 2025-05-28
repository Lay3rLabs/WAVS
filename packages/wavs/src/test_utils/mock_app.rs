use std::sync::Arc;

use crate::{
    apis::{dispatcher::DispatchManager, engine::EngineError},
    dispatcher::Dispatcher,
    engine::{
        mock::{Function, MockEngine},
        runner::{EngineRunner, SingleEngineRunner},
    },
    submission::mock::MockSubmission,
    test_utils::{
        address::{rand_address_evm, rand_event_evm},
        http::{map_response, TestHttpApp},
    },
    trigger_manager::TriggerManager,
    AppContext,
};
use axum::{
    body::Body,
    http::{Method, Request},
};
use serde::{Deserialize, Serialize};
use tower::Service as _;
use utils::{
    config::ChainConfigs,
    telemetry::{DispatcherMetrics, Metrics},
};
use wavs_types::{
    ChainName, ComponentSource, DeleteServicesRequest, IDError, ListServicesResponse, Service,
    ServiceID, ServiceManager, Submit, TriggerAction, TriggerConfig, TriggerData, WorkflowID,
};

use super::mock_trigger_manager::mock_evm_event_trigger;

pub struct MockE2ETestRunner {
    pub ctx: AppContext,
    pub dispatcher: Arc<Dispatcher<SingleEngineRunner<MockEngine>, MockSubmission>>,
    pub http_app: TestHttpApp,
}

impl MockE2ETestRunner {
    pub fn new(ctx: AppContext) -> Arc<Self> {
        // create our dispatcher
        let config = crate::config::Config::default();
        let meter = opentelemetry::global::meter("wavs_metrics");
        let metrics = Metrics::new(&meter);
        let trigger_manager = TriggerManager::new(&config, metrics.wavs.trigger).unwrap();
        let engine = SingleEngineRunner::new(MockEngine::new());
        let submission = MockSubmission::new();
        let storage_path = tempfile::NamedTempFile::new().unwrap();
        let dispatcher = Arc::new(
            Dispatcher::new(
                trigger_manager,
                engine,
                submission,
                ChainConfigs::default(),
                storage_path,
                DispatcherMetrics::default(),
                "https://ipfs.io/ipfs/".to_string(),
            )
            .unwrap(),
        );

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

    pub async fn send_trigger(
        &self,
        service_id: impl TryInto<ServiceID, Error = IDError> + std::fmt::Debug,
        workflow_id: impl TryInto<WorkflowID, Error = IDError> + std::fmt::Debug,
        contract_address: &layer_climb::prelude::Address,
        data: &(impl Serialize + std::fmt::Debug),
        chain_id: impl ToString + std::fmt::Debug,
    ) {
        let sender = self
            .dispatcher
            .trigger_manager
            .action_sender
            .lock()
            .unwrap()
            .clone()
            .unwrap();

        sender
            .send(TriggerAction {
                config: match contract_address {
                    layer_climb::prelude::Address::Evm(_) => TriggerConfig::evm_contract_event(
                        service_id,
                        workflow_id,
                        contract_address.clone().try_into().unwrap(),
                        ChainName::new(chain_id.to_string()).unwrap(),
                        rand_event_evm(),
                    )
                    .unwrap(),
                    layer_climb::prelude::Address::Cosmos { .. } => {
                        TriggerConfig::cosmos_contract_event(
                            service_id,
                            workflow_id,
                            contract_address.clone(),
                            ChainName::new(chain_id.to_string()).unwrap(),
                            rand_event_evm(),
                        )
                        .unwrap()
                    }
                },
                data: TriggerData::new_raw(serde_json::to_string(data).unwrap().as_bytes()),
            })
            .await
            .unwrap();
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
        source: ComponentSource,
        function: impl Function,
    ) {
        self.create_service(service_id, source, function).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_service(
        &self,
        service_id: ServiceID,
        source: ComponentSource,
        function: impl Function,
    ) {
        // "upload" the component
        // not going through http for this because we don't have raw bytes, digest is fake
        let digest = match &source {
            ComponentSource::Download { digest, .. } => digest,
            ComponentSource::Registry { registry } => &registry.digest,
            ComponentSource::Digest(digest) => digest,
        };
        self.dispatcher.engine.engine().register(digest, function);

        // but we can create a service via http router
        let trigger = mock_evm_event_trigger();

        let submit = Submit::evm_contract("evm".try_into().unwrap(), rand_address_evm(), None);

        let service = Service::new_simple(
            service_id,
            Some("mock-service".to_string()),
            trigger,
            source,
            submit,
            ServiceManager::Evm {
                chain_name: "evm".try_into().unwrap(),
                address: rand_address_evm(),
            },
        );

        self.dispatcher.add_service_direct(service).await.unwrap();
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
    fn execute(&self, request: Vec<u8>) -> Result<Option<Vec<u8>>, EngineError> {
        let SquareIn { x } = serde_json::from_slice(&request).unwrap();
        let output = SquareOut { y: x * x };
        Ok(Some(serde_json::to_vec(&output).unwrap()))
    }
}

pub struct ComponentNone;

impl Function for ComponentNone {
    fn execute(&self, _request: Vec<u8>) -> Result<Option<Vec<u8>>, EngineError> {
        Ok(None)
    }
}
