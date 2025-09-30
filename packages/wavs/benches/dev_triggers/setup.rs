use std::{net::SocketAddr, sync::Arc, thread::JoinHandle, time::Duration};

use tempfile::tempdir;
use utils::context::AppContext;
use utils::filesystem::workspace_path;
use wavs::config::Config;
use wavs_types::{
    AllowedHostPermission, Component, ComponentDigest, ComponentSource, Service, WorkflowId,
};
use wavs_types::{SignatureKind, Submit};

#[derive(Debug, Clone)]
pub enum ComponentConfig {
    Default,
    HotLoop { sleep_ms: u32 },
}

pub struct DevTriggersRuntime {
    pub dispatcher: Arc<wavs::dispatcher::Dispatcher<utils::storage::fs::FileStorage>>,
    pub server_addr: SocketAddr,
    pub service: Service,
    pub workflow_id: WorkflowId,
    payload: Vec<u8>,
    ctx: AppContext,
    dispatcher_handle: Option<JoinHandle<()>>,
    wavs_handle: Option<JoinHandle<()>>,
}

impl DevTriggersRuntime {
    const POLL_INTERVAL: Duration = Duration::from_millis(20);

    pub fn new(component_config: ComponentConfig) -> Self {
        // Create temporary directories for clean database state
        let db_dir = tempdir().unwrap();

        // Create config with dev endpoints enabled and proper data directory
        let mut config = Config {
            dev_endpoints_enabled: true,
            data: db_dir.path().to_path_buf(),
            ..Default::default()
        };
        // Provide a test mnemonic so SubmissionManager can create a signer
        config.submission_mnemonic = Some(wavs_types::Credential::new(
            "test test test test test test test test test test test junk".to_string(),
        ));

        // Create a simple workflow
        let workflow_id = WorkflowId::new("dev-trigger-workflow".to_string()).unwrap();

        // Use the real echo_data.wasm component for execution
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("echo_data.wasm");
        let component_bytes = std::fs::read(&component_path).expect("read echo_data.wasm");
        let component_digest = ComponentDigest::hash(&component_bytes);

        // Configure component based on the provided config
        let component_config = match component_config {
            ComponentConfig::Default => std::collections::BTreeMap::new(),
            ComponentConfig::HotLoop { sleep_ms } => std::collections::BTreeMap::from([
                ("sleep-kind".to_string(), "hotloop".to_string()),
                ("sleep-ms".to_string(), sleep_ms.to_string()),
            ]),
        };

        let service = Service {
            name: "Dev Test Service".to_string(),
            workflows: std::collections::BTreeMap::from([(
                workflow_id.clone(),
                wavs_types::Workflow {
                    trigger: wavs_types::Trigger::Manual,
                    component: wavs_types::Component {
                        source: ComponentSource::Digest(component_digest.clone()),
                        permissions: wavs_types::Permissions {
                            file_system: false,
                            allowed_http_hosts: AllowedHostPermission::None,
                        },
                        fuel_limit: None,
                        time_limit_seconds: None,
                        config: component_config,
                        env_keys: std::collections::BTreeSet::new(),
                    },
                    // Use aggregator submit so the submission manager produces packets
                    submit: Submit::Aggregator {
                        url: "http://127.0.0.1:12345".to_string(), // dummy; networking disabled in bench
                        component: Box::new(Component {
                            source: ComponentSource::Digest(component_digest.clone()),
                            permissions: wavs_types::Permissions {
                                file_system: false,
                                allowed_http_hosts: AllowedHostPermission::None,
                            },
                            fuel_limit: None,
                            time_limit_seconds: None,
                            config: std::collections::BTreeMap::new(),
                            env_keys: std::collections::BTreeSet::new(),
                        }),
                        signature_kind: SignatureKind::evm_default(),
                    },
                },
            )]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain: "evm:exec".parse().unwrap(),
                address: Default::default(),
            },
        };

        // Build dispatcher
        #[allow(unused_mut)]
        let mut dispatcher_local = wavs::dispatcher::Dispatcher::new(
            &config,
            utils::telemetry::WavsMetrics::new(opentelemetry::global::meter("wavs-benchmark")),
        )
        .expect("dispatcher new");

        // Disable external networking for benches (debug only)
        dispatcher_local.submission_manager.disable_networking = true;
        dispatcher_local.trigger_manager.disable_networking = true;

        let dispatcher = Arc::new(dispatcher_local);

        // Store component bytes
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("echo_data.wasm");
        let component_bytes = std::fs::read(&component_path).expect("read echo_data.wasm");
        let digest = dispatcher
            .store_component_bytes(component_bytes)
            .expect("store component bytes");
        assert_eq!(digest, component_digest);

        // Register service
        futures::executor::block_on(dispatcher.add_service_direct(service.clone()))
            .expect("add service to dispatcher");

        // Start dispatcher (store handle + context for graceful shutdown)
        let ctx = AppContext::new();
        let d_for_thread = dispatcher.clone();
        let dispatcher_handle = std::thread::spawn({
            let ctx = ctx.clone();
            move || {
                d_for_thread.start(ctx).expect("dispatcher start");
            }
        });

        // Start http server and get address
        let (addr_tx, addr_rx) = std::sync::mpsc::channel();
        let server_config = config.clone();
        let d_for_server = dispatcher.clone();
        let wavs_handle = std::thread::spawn({
            let ctx = ctx.clone();
            move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async move {
                    let health_status = wavs::health::create_shared_health_status();
                    let router = wavs::http::server::make_router(
                        server_config,
                        d_for_server,
                        false,
                        utils::telemetry::HttpMetrics::new(opentelemetry::global::meter(
                            "wavs-benchmark",
                        )),
                        health_status,
                    )
                    .await
                    .unwrap();

                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    let addr = listener.local_addr().unwrap();
                    let _ = addr_tx.send(addr);

                    // Graceful shutdown via AppContext kill signal
                    let mut shutdown_signal = ctx.get_kill_receiver();
                    axum::serve(listener, router)
                        .with_graceful_shutdown(async move {
                            let _ = shutdown_signal.recv().await;
                        })
                        .await
                        .unwrap();
                })
            }
        });

        let server_addr = addr_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("server start timeout");

        DevTriggersRuntime {
            dispatcher,
            server_addr,
            service: service.clone(),
            workflow_id: workflow_id.clone(),
            payload: b"wavs-dev-triggers-bench-payload".to_vec(),
            ctx,
            dispatcher_handle: Some(dispatcher_handle),
            wavs_handle: Some(wavs_handle),
        }
    }

    pub async fn submit_requests(
        &self,
        client: &reqwest::Client,
        n: usize,
        wait_for_completion: bool,
    ) {
        let body = wavs_types::SimulatedTriggerRequest {
            service_id: self.service.id(),
            workflow_id: self.workflow_id.clone(),
            trigger: wavs_types::Trigger::Manual,
            data: wavs_types::TriggerData::Raw(self.payload.clone()),
            count: n,
            wait_for_completion,
        };

        let resp = client
            .post(format!(
                "http://{}:{}/dev/triggers",
                self.server_addr.ip(),
                self.server_addr.port()
            ))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {}
            Ok(r) => panic!("Request failed: {}", r.status()),
            Err(e) => panic!("Request error: {e}"),
        }
    }

    pub async fn wait_for_messages(&self, expected: usize) {
        let mut tick = tokio::time::interval(Self::POLL_INTERVAL);
        loop {
            if self.dispatcher.submission_manager.get_message_count() >= expected as u64 {
                break;
            }
            tick.tick().await;
        }
    }

    pub async fn wait_and_validate_packets(&self, expected: usize) {
        let mut tick = tokio::time::interval(Self::POLL_INTERVAL);
        loop {
            if self.dispatcher.submission_manager.get_debug_packets().len() >= expected {
                break;
            }
            tick.tick().await;
        }

        let packets = self.dispatcher.submission_manager.get_debug_packets();
        assert_eq!(packets.len(), expected);
        for pkt in packets {
            assert_eq!(pkt.envelope.payload.0, &self.payload);
            assert_eq!(pkt.workflow_id, self.workflow_id);
            assert_eq!(pkt.service.id(), self.service.id());
        }
    }
}

impl Drop for DevTriggersRuntime {
    fn drop(&mut self) {
        // Signal shutdown to all async tasks and server
        self.ctx.kill();

        // Join dispatcher and server threads if still running
        if let Some(handle) = self.dispatcher_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.wavs_handle.take() {
            let _ = handle.join();
        }
    }
}
