use std::sync::Arc;

use super::http::{map_response, TestHttpApp};
use axum::{
    body::Body,
    http::{Method, Request},
};
use serde::Serialize;
use tower::Service as _;
use tracing::instrument;
use utils::{
    context::AppContext,
    storage::{fs::FileStorage, memory::MemoryStorage},
    telemetry::{EngineMetrics, Metrics},
};
use utils::{storage::db::RedbStorage, test_utils::address::rand_address_evm};
use wavs::{
    dispatcher::{Dispatcher, DispatcherCommand},
    subsystems::engine::wasm_engine::WasmEngine,
};
use wavs_types::{
    ComponentSource, DeleteServicesRequest, IDError, ListServicesResponse, Service, ServiceID,
    ServiceManager, Submit, WorkflowID,
};

use super::mock_trigger_manager::{mock_evm_event_trigger, mock_real_trigger_action};

pub struct MockE2ETestRunner {
    pub ctx: AppContext,
    pub dispatcher: Arc<Dispatcher<FileStorage>>,
    pub temp_data_dir: tempfile::TempDir,
    pub http_app: TestHttpApp,
}

impl MockE2ETestRunner {
    #[instrument(level = "debug", skip(config, metrics))]
    pub fn create_engine(
        config: Option<wavs::config::Config>,
        metrics: Option<EngineMetrics>,
    ) -> WasmEngine<MemoryStorage> {
        let config = config.unwrap_or_default();
        let memory_storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let metrics = metrics
            .unwrap_or_else(|| EngineMetrics::new(&opentelemetry::global::meter("wavs_metrics")));
        let db_dir = tempfile::tempdir().unwrap();

        WasmEngine::new(
            memory_storage,
            app_data,
            3,
            config.chains.clone(),
            None,
            None,
            metrics,
            RedbStorage::new(db_dir.path()).unwrap(),
        )
    }
    #[instrument(level = "debug", skip(_ctx, data_dir))]
    pub fn create_dispatcher(
        _ctx: AppContext,
        data_dir: impl AsRef<std::path::Path>,
    ) -> Dispatcher<FileStorage> {
        let config = wavs::config::Config {
            submission_mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
            data: data_dir.as_ref().to_path_buf(),
            ..wavs::config::Config::default()
        };
        let meter = opentelemetry::global::meter("wavs_metrics");
        let metrics = Metrics::new(&meter);

        let mut dispatcher = Dispatcher::new(&config, metrics.wavs).unwrap();
        dispatcher.trigger_manager.disable_networking = true;
        dispatcher.submission_manager.disable_networking = true;
        dispatcher
    }

    #[instrument(level = "debug", skip(ctx))]
    pub fn new(ctx: AppContext) -> Arc<Self> {
        let temp_data_dir = tempfile::tempdir().unwrap();
        let dispatcher = Arc::new(Self::create_dispatcher(ctx.clone(), &temp_data_dir));

        // start up the dispatcher in its own thread, before creating any data (similar to how we do it in main)
        std::thread::spawn({
            let ctx = ctx.clone();
            let dispatcher = dispatcher.clone();
            move || {
                dispatcher.start(ctx).unwrap();
            }
        });

        // start up our "http server" in its own thread, before creating any data (similar to how we do it in main)
        let http_app = TestHttpApp::new_with_dispatcher(ctx.clone(), dispatcher.clone(), None);

        Arc::new(Self {
            ctx,
            dispatcher,
            http_app,
            temp_data_dir,
        })
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn send_trigger(
        &self,
        service_id: impl TryInto<ServiceID, Error = IDError> + std::fmt::Debug,
        workflow_id: impl TryInto<WorkflowID, Error = IDError> + std::fmt::Debug,
        contract_address: &layer_climb::prelude::Address,
        data: &(impl Serialize + std::fmt::Debug),
        chain_id: impl ToString + std::fmt::Debug,
    ) {
        self.dispatcher
            .trigger_manager
            .send_dispatcher_commands([DispatcherCommand::Trigger(mock_real_trigger_action(
                service_id,
                workflow_id,
                contract_address,
                data,
                chain_id,
            ))])
            .await
            .unwrap();
    }

    #[instrument(level = "debug", skip(self))]
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

    #[instrument(level = "debug", skip(self))]
    pub async fn create_service(&self, service_id: ServiceID, component_source: ComponentSource) {
        // but we can create a service via http router
        let trigger = mock_evm_event_trigger();

        let submit = Submit::None;

        let service = Service::new_simple(
            service_id,
            Some("mock-service".to_string()),
            trigger,
            component_source,
            submit,
            ServiceManager::Evm {
                chain_name: "evm".try_into().unwrap(),
                address: rand_address_evm(),
            },
        );

        self.dispatcher.add_service_direct(service).await.unwrap();
    }

    #[instrument(level = "debug", skip(self))]
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
