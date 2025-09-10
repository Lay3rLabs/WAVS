use std::{net::SocketAddr, sync::Arc, time::Duration};

use tempfile::tempdir;
use utils::context::AppContext;
use utils::filesystem::workspace_path;
use wavs::config::Config;
use wavs_types::{
    AllowedHostPermission, Component, ComponentDigest, ComponentSource, Service, WorkflowId,
};
use wavs_types::{SignatureKind, Submit};

pub struct DevTriggersRuntime {
    pub dispatcher: Arc<wavs::dispatcher::Dispatcher<utils::storage::fs::FileStorage>>,
    pub server_addr: SocketAddr,
    pub service: Service,
    pub workflow_id: WorkflowId,
    payload: Vec<u8>,
}

pub struct DevTriggersSetup {
    pub config: Config,
    pub service: Service,
    pub workflow_id: WorkflowId,
    pub _temp_dir: tempfile::TempDir,
    pub _db_dir: tempfile::TempDir,
    pub expected_component_digest: wavs_types::ComponentDigest,
}

impl DevTriggersSetup {
    pub fn new() -> Arc<Self> {
        // Create temporary directories for clean database state
        let temp_dir = tempdir().unwrap();
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
                        config: std::collections::BTreeMap::new(),
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

        Arc::new(Self {
            config,
            service,
            workflow_id,
            _temp_dir: temp_dir,
            _db_dir: db_dir,
            expected_component_digest: component_digest,
        })
    }

    /// Boot a dispatcher + HTTP server, register service, and return a runtime handle
    pub fn start_runtime(self: &Arc<Self>) -> Arc<DevTriggersRuntime> {
        // Build dispatcher
        #[allow(unused_mut)]
        let mut dispatcher_local = wavs::dispatcher::Dispatcher::new(
            &self.config,
            utils::telemetry::WavsMetrics::new(&opentelemetry::global::meter("wavs-benchmark")),
        )
        .expect("dispatcher new");

        // Disable external networking for benches (debug only)
        #[cfg(debug_assertions)]
        {
            dispatcher_local.submission_manager.disable_networking = true;
            dispatcher_local.trigger_manager.disable_networking = true;
        }

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
        assert_eq!(digest, self.expected_component_digest);

        // Register service
        futures::executor::block_on(dispatcher.add_service_direct(self.service.clone()))
            .expect("add service to dispatcher");

        // Start dispatcher
        let ctx = AppContext::new();
        let d_for_thread = dispatcher.clone();
        std::thread::spawn(move || {
            d_for_thread.start(ctx).expect("dispatcher start");
        });

        // Start http server and get address
        let (addr_tx, addr_rx) = std::sync::mpsc::channel();
        let server_config = self.config.clone();
        let d_for_server = dispatcher.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let router = wavs::http::server::make_router(
                    server_config,
                    d_for_server,
                    false,
                    utils::telemetry::HttpMetrics::new(&opentelemetry::global::meter(
                        "wavs-benchmark",
                    )),
                )
                .await
                .unwrap();

                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = listener.local_addr().unwrap();
                let _ = addr_tx.send(addr);
                axum::serve(listener, router).await.unwrap();
            })
        });

        let server_addr = addr_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("server start timeout");

        Arc::new(DevTriggersRuntime {
            dispatcher,
            server_addr,
            service: self.service.clone(),
            workflow_id: self.workflow_id.clone(),
            payload: b"wavs-dev-triggers-bench-payload".to_vec(),
        })
    }
}

impl DevTriggersRuntime {
    const WAIT_TIMEOUT: Duration = Duration::from_secs(5);
    const POLL_INTERVAL: Duration = Duration::from_millis(20);

    pub async fn submit_requests(&self, client: &reqwest::Client, n: usize) {
        let body = wavs_types::SimulatedTriggerRequest {
            service_id: self.service.id(),
            workflow_id: self.workflow_id.clone(),
            trigger: wavs_types::Trigger::Manual,
            data: wavs_types::TriggerData::Raw(self.payload.clone()),
            count: n,
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
        tokio::time::timeout(Self::WAIT_TIMEOUT, async {
            loop {
                if self.dispatcher.submission_manager.get_message_count() >= expected as u64 {
                    break;
                }
                tick.tick().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Timed out waiting for {} messages", expected));
    }

    #[cfg(debug_assertions)]
    pub async fn wait_and_validate_packets(&self, expected: usize) {
        let mut tick = tokio::time::interval(Self::POLL_INTERVAL);
        tokio::time::timeout(Self::WAIT_TIMEOUT, async {
            loop {
                if self.dispatcher.submission_manager.get_debug_packets().len() >= expected {
                    break;
                }
                tick.tick().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Timed out waiting for {} packets", expected));

        let packets = self.dispatcher.submission_manager.get_debug_packets();
        assert_eq!(packets.len(), expected);
        for pkt in packets {
            assert_eq!(pkt.envelope.payload.0, &self.payload);
            assert_eq!(pkt.workflow_id, self.workflow_id);
            assert_eq!(pkt.service.id(), self.service.id());
        }
    }
}
