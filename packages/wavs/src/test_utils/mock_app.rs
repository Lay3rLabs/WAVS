use std::sync::Arc;

use crate::{
    dispatcher::Dispatcher,
    subsystems::engine::wasm_engine::WasmEngine,
    test_utils::{
        address::{rand_address_evm, rand_event_evm},
        http::{map_response, TestHttpApp},
    },
    AppContext,
};
use alloy_primitives::LogData;
use axum::{
    body::Body,
    http::{Method, Request},
};
use serde::Serialize;
use tower::Service as _;
use utils::{
    storage::{fs::FileStorage, memory::MemoryStorage},
    telemetry::{EngineMetrics, Metrics},
};
use wavs_types::{
    ChainName, ComponentSource, DeleteServicesRequest, IDError, ListServicesResponse, Service,
    ServiceID, ServiceManager, Submit, TriggerAction, TriggerConfig, TriggerData, WorkflowID,
};

use super::{address::rand_event_cosmos, mock_trigger_manager::mock_evm_event_trigger};

pub struct MockE2ETestRunner {
    pub ctx: AppContext,
    pub dispatcher: Arc<Dispatcher<FileStorage>>,
    pub temp_data_dir: tempfile::TempDir,
    pub http_app: TestHttpApp,
}

impl MockE2ETestRunner {
    pub fn create_engine(
        config: Option<crate::config::Config>,
        metrics: Option<EngineMetrics>,
    ) -> WasmEngine<MemoryStorage> {
        let config = config.unwrap_or_default();
        let memory_storage = MemoryStorage::new();
        let app_data = tempfile::tempdir().unwrap();
        let metrics = metrics
            .unwrap_or_else(|| EngineMetrics::new(&opentelemetry::global::meter("wavs_metrics")));
        WasmEngine::new(
            memory_storage,
            app_data,
            3,
            config.chains.clone(),
            None,
            None,
            metrics,
        )
    }
    pub fn create_dispatcher(
        _ctx: AppContext,
        data_dir: impl AsRef<std::path::Path>,
    ) -> Dispatcher<FileStorage> {
        // create our dispatcher
        let config = crate::config::Config {
            submission_mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
            data: data_dir.as_ref().to_path_buf(),
            ..crate::config::Config::default()
        };
        let meter = opentelemetry::global::meter("wavs_metrics");
        let metrics = Metrics::new(&meter);
        Dispatcher::new(&config, metrics.wavs).unwrap()
    }

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

        let data = serde_json::to_vec(data).unwrap();
        match contract_address {
            layer_climb::prelude::Address::Evm(_) => {
                let event = rand_event_evm();

                sender
                    .send(TriggerAction {
                        config: TriggerConfig::evm_contract_event(
                            service_id,
                            workflow_id,
                            contract_address.clone().try_into().unwrap(),
                            ChainName::new(chain_id.to_string()).unwrap(),
                            event,
                        )
                        .unwrap(),
                        data: TriggerData::EvmContractEvent {
                            contract_address: contract_address.clone().try_into().unwrap(),
                            chain_name: ChainName::new(chain_id.to_string()).unwrap(),
                            // FIXME: this should be a proper EVM event, this is just a placeholder
                            log: LogData::new(vec![event.into_inner().into()], data.into())
                                .unwrap(),
                            block_height: 1,
                        },
                    })
                    .await
                    .unwrap();
            }
            layer_climb::prelude::Address::Cosmos { .. } => {
                let event = rand_event_cosmos();

                sender
                    .send(TriggerAction {
                        config: TriggerConfig::cosmos_contract_event(
                            service_id,
                            workflow_id,
                            contract_address.clone(),
                            ChainName::new(chain_id.to_string()).unwrap(),
                            event.clone(),
                        )
                        .unwrap(),
                        data: TriggerData::CosmosContractEvent {
                            contract_address: contract_address.clone(),
                            chain_name: ChainName::new(chain_id.to_string()).unwrap(),
                            event: cosmwasm_std::Event::new("new-message").add_attributes(vec![
                                ("id", "1".to_string()),
                                ("data", const_hex::encode(data)),
                            ]),
                            block_height: 1,
                        },
                    })
                    .await
                    .unwrap();
            }
        }
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

    #[allow(clippy::too_many_arguments)]
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
