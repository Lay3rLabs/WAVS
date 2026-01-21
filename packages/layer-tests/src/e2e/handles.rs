mod cosmos;
mod evm;
pub mod hypercore;

use std::{collections::HashMap, io, net::SocketAddr, sync::Arc, time::Duration};

use cosmos::CosmosInstance;
use evm::EvmInstance;
use utils::{
    context::AppContext,
    telemetry::Metrics,
    test_utils::middleware::{
        cosmos::{CosmosMiddleware, CosmosMiddlewareKind},
        evm::EvmMiddleware,
    },
};
use wavs::dispatcher::Dispatcher;
use wavs::subsystems::aggregator::p2p::P2pConfig;
use wavs_cli::clients::HttpClient;
use wavs_types::{ChainKey, ChainKeyNamespace};

use crate::config::TestP2pMode;

use super::config::Configs;
use super::matrix::EvmService;

/// Check if a port is available for binding
fn check_port_availability(port: u16) -> bool {
    match std::net::TcpListener::bind(format!("127.0.0.1:{}", port)) {
        Ok(_) => true,
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            tracing::warn!("Port {} is already in use", port);
            false
        }
        Err(_) => false,
    }
}

pub struct AppHandles {
    /// One handle per WAVS operator instance
    pub wavs_handles: Vec<std::thread::JoinHandle<()>>,
    pub evm_middleware: Option<EvmMiddleware>,
    pub cosmos_middlewares: CosmosMiddlewares,
    _evm_chains: Vec<EvmInstance>,
    _cosmos_chains: Vec<CosmosInstance>,
    _hyperswarm_bootstrap: Option<async_std::task::JoinHandle<std::io::Result<()>>>,
}

pub type CosmosMiddlewares = Arc<HashMap<ChainKey, CosmosMiddleware>>;

impl AppHandles {
    pub fn start(ctx: &AppContext, configs: &mut Configs, metrics: Metrics) -> Self {
        let (bootstrap_addr, bootstrap_handle) =
            if configs.matrix.evm.contains(&EvmService::HypercoreEchoData) {
                Self::start_hyperswarm_bootstrap()
            } else {
                (None, None)
            };
        if let Some(addr) = bootstrap_addr {
            let addr = addr.to_string();
            for wavs_config in configs.wavs_configs.iter_mut() {
                wavs_config.hyperswarm_bootstrap = Some(addr.clone());
            }
        }

        let mut evm_chains = Vec::new();
        let mut cosmos_chains = Vec::new();

        let mut cosmos_middlewares = HashMap::new();
        {
            let chains = configs.chains.read().unwrap();
            for chain_config in chains.evm_iter() {
                let handle = EvmInstance::spawn(ctx.clone(), configs, chain_config.clone());
                evm_chains.push(handle);
            }

            for (index, chain_config) in chains.cosmos_iter().enumerate() {
                let handle =
                    CosmosInstance::spawn(ctx.clone(), configs, chain_config.clone(), index);

                let chain_key = ChainKey {
                    namespace: ChainKeyNamespace::COSMOS.parse().unwrap(),
                    id: chain_config.chain_id.clone(),
                };
                let middleware = ctx
                    .rt
                    .block_on(CosmosMiddleware::new(
                        chain_config.clone(),
                        CosmosMiddlewareKind::Mock,
                        configs.mnemonics.cosmos_middleware[index].to_string(),
                    ))
                    .unwrap();

                cosmos_middlewares.insert(chain_key, middleware);
                cosmos_chains.push(handle);
            }
        }

        // Spawn one WAVS instance per operator
        let mut wavs_handles = Vec::with_capacity(configs.num_operators());

        // Check if we're using Remote P2P mode (Kademlia)

        if configs.p2p == TestP2pMode::Kademlia && configs.num_operators() > 1 {
            // Remote mode: start operator 0 first, get bootstrap address, then start others
            wavs_handles = Self::start_wavs_remote_mode(ctx, configs, &metrics)
                .expect("Failed to start operators in remote mode");
        } else {
            // Local mode or single operator: start all at once
            for (operator_index, wavs_config) in configs.wavs_configs.iter().enumerate() {
                let handle = Self::spawn_wavs_operator(ctx, wavs_config, &metrics, operator_index);
                wavs_handles.push(handle);
            }
        }

        let evm_middleware = if evm_chains.is_empty() {
            None
        } else {
            Some(EvmMiddleware::new(configs.evm_middleware_type).unwrap())
        };

        Self {
            wavs_handles,
            evm_middleware,
            cosmos_middlewares: Arc::new(cosmos_middlewares),
            _evm_chains: evm_chains,
            _cosmos_chains: cosmos_chains,
            _hyperswarm_bootstrap: bootstrap_handle,
        }
    }

    pub fn try_join(self) -> Vec<std::thread::Result<()>> {
        let mut results = Vec::new();
        for handle in self.wavs_handles {
            results.push(handle.join());
        }
        results
    }

    /// Spawn a single WAVS operator
    fn spawn_wavs_operator(
        ctx: &AppContext,
        wavs_config: &wavs::config::Config,
        metrics: &Metrics,
        operator_index: usize,
    ) -> std::thread::JoinHandle<()> {
        let dispatcher = Arc::new(Dispatcher::new(wavs_config, metrics.wavs.clone()).unwrap());

        std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = wavs_config.clone();
            let http_metrics = metrics.http.clone();

            move || {
                tracing::info!(
                    "Starting WAVS operator {} on port {}",
                    operator_index,
                    config.port
                );
                let health_status = wavs::health::SharedHealthStatus::new();
                wavs::run_server(ctx, config, dispatcher, http_metrics, health_status);
            }
        })
    }

    /// Start WAVS operators in Remote P2P mode (Kademlia)
    /// Operator 0 starts first as bootstrap server, others connect to it
    fn start_wavs_remote_mode(
        ctx: &AppContext,
        configs: &Configs,
        metrics: &Metrics,
    ) -> Result<Vec<std::thread::JoinHandle<()>>, anyhow::Error> {
        let mut handles = Vec::with_capacity(configs.num_operators());

        // Log port availability for better diagnostics
        for (idx, config) in configs.wavs_configs.iter().enumerate() {
            let http_port = config.port as u16;
            let p2p_port = match &config.p2p {
                P2pConfig::Remote { listen_port, .. } => *listen_port,
                P2pConfig::Local { listen_port, .. } => *listen_port,
                P2pConfig::Disabled => continue,
            };

            tracing::info!(
                "Operator {} - HTTP port {} availability: {}, P2P port {} availability: {}",
                idx,
                http_port,
                check_port_availability(http_port),
                p2p_port,
                check_port_availability(p2p_port)
            );
        }

        // Start operator 0 (bootstrap server)
        let op0_config = &configs.wavs_configs[0];
        tracing::info!("Starting operator 0 as bootstrap server");
        handles.push(Self::spawn_wavs_operator(ctx, op0_config, metrics, 0));

        // Wait for operator 0 to be ready and get its bootstrap address
        let op0_url = format!("http://127.0.0.1:{}", op0_config.port);
        let bootstrap_addr = ctx.rt.block_on(async {
            let client = HttpClient::new(op0_url);

            // Wait for the server to be ready
            tokio::time::sleep(Duration::from_millis(500)).await;

            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(30); // Increased from 6s to 30s
            let mut delay = Duration::from_millis(100);

            loop {
                match client.get_p2p_status().await {
                    Ok(status) => {
                        // Prefer external_addresses, fall back to listen_addresses
                        let addr = status
                            .external_addresses
                            .first()
                            .or(status.listen_addresses.first())
                            .cloned();

                        if let Some(addr) = addr {
                            tracing::info!("Got bootstrap address from operator 0: {}", addr);
                            return Ok(addr);
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Waiting for operator 0 P2P status: {:?}", e);
                    }
                }

                if start.elapsed() >= timeout {
                    return Err(anyhow::anyhow!(
                        "Timed out waiting for operator 0 bootstrap address after 30s"
                    ));
                }

                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(5)); // Exponential backoff, max 5s
            }
        })?;

        // Start remaining operators with the bootstrap address
        for (operator_index, wavs_config) in configs.wavs_configs.iter().enumerate().skip(1) {
            // Clone and modify config to add bootstrap address
            let mut config = wavs_config.clone();
            if let P2pConfig::Remote {
                listen_port,
                bootstrap_nodes: _,
                max_retry_duration_secs,
                retry_interval_ms,
                submission_ttl_secs,
                max_catchup_submissions,
                cleanup_interval_secs,
                kademlia_discovery_interval_secs,
            } = &config.p2p
            {
                config.p2p = P2pConfig::Remote {
                    listen_port: *listen_port,
                    bootstrap_nodes: vec![bootstrap_addr.clone()],
                    max_retry_duration_secs: *max_retry_duration_secs,
                    retry_interval_ms: *retry_interval_ms,
                    submission_ttl_secs: *submission_ttl_secs,
                    max_catchup_submissions: *max_catchup_submissions,
                    cleanup_interval_secs: *cleanup_interval_secs,
                    kademlia_discovery_interval_secs: *kademlia_discovery_interval_secs,
                };
            }

            tracing::info!(
                "Starting operator {} with bootstrap: {}",
                operator_index,
                bootstrap_addr
            );
            handles.push(Self::spawn_wavs_operator(
                ctx,
                &config,
                metrics,
                operator_index,
            ));
        }

        Ok(handles)
    }

    fn start_hyperswarm_bootstrap() -> (
        Option<SocketAddr>,
        Option<async_std::task::JoinHandle<std::io::Result<()>>>,
    ) {
        // Bind to 0.0.0.0 to listen on all interfaces, but announce the Docker bridge IP
        // This matches how P2P works: it binds to 0.0.0.0 but peers connect via 172.17.0.1
        let bind_addr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 49737);

        match async_std::task::block_on(hyperswarm::run_bootstrap_node(Some(bind_addr))) {
            Ok((addr, handle)) => {
                // Announce the Docker bridge IP (172.17.0.1) instead of 127.0.0.1
                // This works both locally (via Docker Desktop/route) and in CI
                let announce_addr = SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::new(172, 17, 0, 1)),
                    addr.port()
                );

                tracing::info!(
                    "Started hyperswarm bootstrap node at {} (announcing {})",
                    addr,
                    announce_addr
                );

                // Give the bootstrap node time to start listening
                std::thread::sleep(Duration::from_secs(1));

                (Some(announce_addr), Some(handle))
            }
            Err(err) => {
                tracing::warn!("Failed to start hyperswarm bootstrap node: {err}");
                (None, None)
            }
        }
    }
}
