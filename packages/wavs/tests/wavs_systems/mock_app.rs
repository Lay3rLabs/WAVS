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
    service::DEFAULT_IPFS_GATEWAY,
    storage::{fs::FileStorage, memory::MemoryStorage},
    telemetry::{EngineMetrics, Metrics},
};
use utils::{storage::db::RedbStorage, test_utils::address::rand_address_evm};
use wavs::{
    dispatcher::{Dispatcher, DispatcherCommand},
    subsystems::engine::wasm_engine::WasmEngine,
};
use wavs_types::{
    ChainKey, ChainKeyError, Component, ComponentSource, Credential, DeleteServicesRequest,
    ListServicesResponse, Service, ServiceId, ServiceManager, SignatureKind, Submit, WorkflowId,
    WorkflowIdError,
};

use super::mock_trigger_manager::{mock_evm_event_trigger, mock_real_trigger_action};

pub struct MockE2ETestRunner {
    pub ctx: AppContext,
    pub dispatcher: Arc<Dispatcher<FileStorage>>,
    pub temp_data_dir: tempfile::TempDir,
    pub http_app: TestHttpApp,
}

impl MockE2ETestRunner {
    #[instrument(skip(config, metrics))]
    pub fn create_engine(
        config: Option<wavs::config::Config>,
        metrics: Option<EngineMetrics>,
    ) -> WasmEngine<MemoryStorage> {
        let config = config.unwrap_or_default();
        let memory_storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let metrics = metrics
            .unwrap_or_else(|| EngineMetrics::new(opentelemetry::global::meter("wavs_metrics")));
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
            DEFAULT_IPFS_GATEWAY.to_owned(),
        )
    }
    #[instrument(skip(_ctx, data_dir))]
    pub fn create_dispatcher(
        _ctx: AppContext,
        data_dir: impl AsRef<std::path::Path>,
    ) -> Dispatcher<FileStorage> {
        let config = wavs::config::Config {
            submission_mnemonic: Some(Credential::new(
                "test test test test test test test test test test test junk".to_string(),
            )),
            data: data_dir.as_ref().to_path_buf(),
            ..wavs::config::Config::default()
        };
        let meter = opentelemetry::global::meter("wavs_metrics");
        let metrics = Metrics::new(meter);

        let mut dispatcher = Dispatcher::new(&config, metrics.wavs).unwrap();
        dispatcher.trigger_manager.disable_networking = true;
        dispatcher.submission_manager.disable_networking = true;
        dispatcher
    }

    #[instrument(skip(ctx))]
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

    #[instrument(skip(self))]
    pub fn send_trigger(
        &self,
        service_id: ServiceId,
        workflow_id: impl TryInto<WorkflowId, Error = WorkflowIdError> + std::fmt::Debug,
        contract_address: &layer_climb::prelude::Address,
        data: &(impl Serialize + std::fmt::Debug),
        chain: impl TryInto<ChainKey, Error = ChainKeyError> + std::fmt::Debug + Clone,
    ) {
        self.dispatcher
            .trigger_manager
            .send_dispatcher_commands([DispatcherCommand::Trigger(mock_real_trigger_action(
                service_id,
                workflow_id,
                contract_address,
                data,
                chain,
            ))])
            .unwrap();
    }

    #[instrument(skip(self))]
    pub async fn list_services(&self) -> ListServicesResponse {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/services")
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

    #[instrument(skip(self))]
    pub async fn create_service(
        &self,
        name: Option<String>,
        component_source: ComponentSource,
    ) -> ServiceId {
        // but we can create a service via http router
        let trigger = mock_evm_event_trigger();

        let submit = Submit::Aggregator {
            url: "http://example.com".to_string(),
            // just use the same component for submit for simplicity
            component: Box::new(Component::new(component_source.clone())),
            signature_kind: SignatureKind::evm_default(),
        };

        let service = Service::new_simple(
            name,
            trigger,
            component_source,
            submit,
            ServiceManager::Evm {
                chain: "evm:anvil".try_into().unwrap(),
                address: rand_address_evm(),
            },
        );

        let service_id = service.id();

        self.dispatcher.add_service_direct(service).await.unwrap();

        service_id
    }

    #[instrument(skip(self))]
    pub async fn delete_services(&self, service_managers: Vec<ServiceManager>) {
        let body = serde_json::to_string(&DeleteServicesRequest { service_managers }).unwrap();

        let req = Request::builder()
            .method(Method::DELETE)
            .header("Content-Type", "application/json")
            .uri("/services")
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
