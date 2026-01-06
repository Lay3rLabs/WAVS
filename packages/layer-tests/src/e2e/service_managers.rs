use std::{collections::HashMap, sync::Arc};

use futures::{stream::FuturesUnordered, StreamExt};
use utils::test_utils::{
    middleware::{
        cosmos::CosmosServiceManager,
        evm::{EvmMiddleware, MiddlewareServiceManagerConfig},
        operator::AvsOperator,
    },
    mock_service_manager::MockEvmServiceManager,
};
use wavs_cli::{clients::HttpClient, command::deploy_service::DeployService};
use wavs_types::{
    ChainKey, ChainKeyNamespace, Service, ServiceManager, ServiceStatus, SignerResponse,
};

use crate::{
    deployment::ServiceDeployment,
    e2e::{handles::CosmosMiddlewares, helpers::wait_for_evm_trigger_streams_to_finalize},
};

use crate::e2e::{
    clients::Clients,
    components::ComponentSources,
    config::Configs,
    helpers::create_service_for_test,
    test_registry::{CosmosCodeMap, TestRegistry},
};

#[derive(Clone)]
pub struct ServiceManagers {
    configs: Arc<Configs>,
    lookup: Arc<HashMap<String, AnyServiceManagerInstance>>,
}

pub enum AnyServiceManagerInstance {
    Evm {
        chain: ChainKey,
        manager: MockEvmServiceManager,
    },
    Cosmos {
        chain: ChainKey,
        manager: CosmosServiceManager,
    },
}

impl ServiceManagers {
    pub fn new(configs: Configs) -> Self {
        Self {
            lookup: Arc::new(HashMap::new()),
            configs: Arc::new(configs),
        }
    }
}

impl ServiceManagers {
    pub async fn bootstrap(
        &mut self,
        registry: &TestRegistry,
        clients: &Clients,
        evm_middleware: Option<EvmMiddleware>,
        cosmos_middlewares: CosmosMiddlewares,
    ) {
        tracing::warn!("WAVS Concurrency: {}", self.configs.wavs_concurrency);
        tracing::warn!(
            "Middleware Concurrency: {}",
            self.configs.middleware_concurrency
        );
        tracing::warn!("Bootstrapping service managers...");
        self.deploy_service_managers(registry, clients, evm_middleware, cosmos_middlewares)
            .await;
        tracing::warn!("Bootstrapping initial service uris...");
        self.set_initial_service_uris(registry, clients).await;
        tracing::warn!("Bootstrapping initial services...");
        self.deploy_initial_wavs_services(registry, clients).await;
        tracing::warn!("Bootstrapping register operators...");
        self.register_operators(registry, clients).await;
    }

    pub fn get_service_manager(&self, test_name: &str) -> ServiceManager {
        match self.lookup.get(test_name).unwrap() {
            AnyServiceManagerInstance::Evm { chain, manager } => ServiceManager::Evm {
                chain: chain.clone(),
                address: manager.address(),
            },
            AnyServiceManagerInstance::Cosmos { chain, manager } => ServiceManager::Cosmos {
                chain: chain.clone(),
                address: manager.address.clone(),
            },
        }
    }

    pub async fn deploy_service_managers(
        &mut self,
        registry: &TestRegistry,
        clients: &Clients,
        evm_middleware: Option<EvmMiddleware>,
        cosmos_middlewares: CosmosMiddlewares,
    ) {
        let mut lookup = HashMap::new();

        let mut futures = Vec::new();

        for test in registry.list_all() {
            let chain = test
                .service_manager_chain
                .clone()
                .unwrap_or_else(|| panic!("missing service manager chain for test {}", test.name));
            futures.push({
                let evm_middleware = evm_middleware.clone();
                let cosmos_middlewares = cosmos_middlewares.clone();
                async move {
                    match chain.namespace.as_str() {
                        ChainKeyNamespace::EVM => {
                            let wallet_client = clients.get_evm_client(&chain);
                            let test_name = test.name.clone();
                            let middleware = evm_middleware.clone().unwrap();
                            tracing::info!("Deploying service manager for test {}", test_name);
                            let manager = MockEvmServiceManager::new(middleware, wallet_client)
                                .await
                                .unwrap();
                            tracing::info!(
                                "EVM Service manager for test {} is {}",
                                test_name,
                                manager.address()
                            );
                            (test_name, AnyServiceManagerInstance::Evm { manager, chain })
                        }
                        ChainKeyNamespace::COSMOS => {
                            let middleware = cosmos_middlewares.get(&chain).unwrap();
                            let manager = middleware.deploy_service_manager().await.unwrap();
                            tracing::info!(
                                "Cosmos Service manager for test {} is {}",
                                test.name,
                                manager.address
                            );
                            (
                                test.name.clone(),
                                AnyServiceManagerInstance::Cosmos { manager, chain },
                            )
                        }
                        other => panic!("Unsupported chain namespace: {}", other),
                    }
                }
            });
        }

        tracing::info!("Deploying {} service managers", futures.len());

        if self.configs.middleware_concurrency {
            let mut futures_unordered = FuturesUnordered::from_iter(futures);
            while let Some((test_name, value)) = futures_unordered.next().await {
                if lookup.insert(test_name.clone(), value).is_some() {
                    panic!("Service manager for test {} already exists", test_name);
                }
            }
        } else {
            for future in futures {
                let (test_name, value) = future.await;
                if lookup.insert(test_name.clone(), value).is_some() {
                    panic!("Service manager for test {} already exists", test_name);
                }
            }
        }

        self.lookup = Arc::new(lookup);
    }

    pub async fn set_initial_service_uris(&self, registry: &TestRegistry, clients: &Clients) {
        let mut futures = Vec::new();

        for test in registry.list_all() {
            let service_manager = self.get_service_manager(&test.name);

            let service = Service {
                name: test.name.to_string(),
                workflows: Default::default(),
                status: ServiceStatus::Paused,
                manager: service_manager,
            };

            // Save the service on WAVS endpoint (just a local test thing, real-world would be IPFS or similar)
            let service_url = DeployService::save_service(&clients.cli_ctx, &service)
                .await
                .unwrap();

            let service_manager_instance = self.lookup.get(&test.name).unwrap();

            futures.push(async move {
                match service_manager_instance {
                    AnyServiceManagerInstance::Evm { manager, .. } => {
                        manager.set_service_uri(service_url).await.unwrap();
                    }
                    AnyServiceManagerInstance::Cosmos { manager, .. } => {
                        manager.set_service_uri(&service_url).await.unwrap();
                    }
                }
            });
        }

        if self.configs.middleware_concurrency {
            futures::future::join_all(futures).await;
        } else {
            for future in futures {
                future.await;
            }
        }
    }

    pub async fn deploy_initial_wavs_services(
        &mut self,
        registry: &TestRegistry,
        clients: &Clients,
    ) {
        let mut futures = Vec::new();

        for test in registry.list_all() {
            let service_manager = self.get_service_manager(&test.name);
            let http_clients = clients.http_clients.clone();

            futures.push(async move {
                tracing::info!("Deploying service {} on all WAVS instances", test.name);

                // Deploy the service on ALL WAVS instances
                for (idx, http_client) in http_clients.iter().enumerate() {
                    tracing::info!("Deploying service {} on WAVS instance {}", test.name, idx);
                    http_client
                        .create_service(service_manager.clone(), None)
                        .await
                        .unwrap();
                }
            });
        }

        if self.configs.wavs_concurrency {
            let mut futures_unordered = FuturesUnordered::from_iter(futures);
            while (futures_unordered.next().await).is_some() {}
        } else {
            for future in futures {
                future.await;
            }
        }
    }

    pub async fn register_operators(&self, registry: &TestRegistry, _clients: &Clients) {
        use crate::e2e::config::{MULTI_OPERATOR_COUNT, WAVS_BASE_PORT};

        let mut futures = Vec::new();

        for (test_index, test) in registry.list_all().enumerate() {
            let service_manager = self.get_service_manager(&test.name);

            // For multi-operator tests, register all operators with 2/3 quorum requirement
            let num_operators = if test.multi_operator {
                MULTI_OPERATOR_COUNT
            } else {
                1
            };

            // Collect all operators for this test
            let mut avs_operators = Vec::with_capacity(num_operators);

            for operator_offset in 0..num_operators {
                // Get the signing key from the correct WAVS instance
                // Each WAVS instance runs on its own port: 8000, 8001, 8002, etc.
                let wavs_port = WAVS_BASE_PORT + operator_offset as u32;
                let wavs_endpoint = format!("http://127.0.0.1:{}", wavs_port);
                let http_client = HttpClient::new(wavs_endpoint);

                let SignerResponse::Secp256k1 {
                    evm_address: avs_signer_address,
                    hd_index: wavs_signer_hd_index,
                } = http_client
                    .get_service_signer(service_manager.clone())
                    .await
                    .unwrap();

                // unique HD index per test and operator to avoid nonce collisions
                let operator_hd_index =
                    (test_index * MULTI_OPERATOR_COUNT + operator_offset) as u32;
                let operator_mnemonic = &self.configs.mnemonics.operators[operator_offset];
                let operator_signer = utils::evm_client::signing::make_signer(
                    operator_mnemonic,
                    Some(operator_hd_index),
                )
                .unwrap();
                let operator_address = operator_signer.address();
                let operator_private_key = const_hex::encode(operator_signer.to_bytes());

                // Get the signing key that this WAVS instance will use
                let signing_signer = utils::evm_client::signing::make_signer(
                    operator_mnemonic,
                    Some(wavs_signer_hd_index),
                )
                .unwrap();
                let signing_address = signing_signer.address();
                let signing_private_key = const_hex::encode(signing_signer.to_bytes());

                assert_eq!(
                    signing_address.to_string().to_lowercase(),
                    avs_signer_address.to_lowercase(),
                    "Derived signing address doesn't match WAVS signer address for operator {}",
                    operator_offset
                );

                let avs_operator = AvsOperator::with_keys(
                    operator_address,
                    signing_address,
                    operator_private_key,
                    signing_private_key,
                );

                avs_operators.push(avs_operator);
            }

            // Calculate required operators for quorum (2/3 of total)
            let required_to_pass = if test.multi_operator {
                // For 2/3 quorum with 3 operators, we need 2 signatures
                ((num_operators as u64) * 2 + 2) / 3 // Ceiling division for 2/3
            } else {
                1
            };

            let service_manager_instance = self.lookup.get(&test.name).unwrap();
            futures.push(async move {
                match service_manager_instance {
                    AnyServiceManagerInstance::Evm { manager, .. } => {
                        manager
                            .configure(&MiddlewareServiceManagerConfig::new(
                                &avs_operators,
                                required_to_pass,
                            ))
                            .await
                            .unwrap();
                    }
                    AnyServiceManagerInstance::Cosmos { manager, .. } => {
                        // For Cosmos, register each operator individually
                        for operator in avs_operators {
                            manager.register_operator(operator.clone()).await.unwrap();
                        }
                    }
                }
            });
        }

        if self.configs.middleware_concurrency {
            let mut futures_unordered = FuturesUnordered::from_iter(futures);
            while (futures_unordered.next().await).is_some() {}
        } else {
            for future in futures {
                future.await;
            }
        }
    }

    pub async fn create_real_wavs_services(
        &mut self,
        registry: &TestRegistry,
        clients: &Clients,
        component_sources: &ComponentSources,
        cosmos_code_map: CosmosCodeMap,
    ) -> HashMap<String, ServiceDeployment> {
        let mut futures = Vec::new();

        for test in registry.list_all() {
            let service_manager = self.get_service_manager(&test.name);

            futures.push(create_service_for_test(
                test,
                clients,
                component_sources,
                service_manager,
                cosmos_code_map.clone(),
            ));
        }

        let mut services = HashMap::new();

        if self.configs.wavs_concurrency {
            let mut futures_unordered = FuturesUnordered::from_iter(futures);
            while let Some(deployment_result) = futures_unordered.next().await {
                services.insert(deployment_result.service.name.clone(), deployment_result);
            }
        } else {
            for future in futures {
                let deployment_result = future.await;
                services.insert(deployment_result.service.name.clone(), deployment_result);
            }
        }

        services
    }

    pub async fn update_services(&self, clients: &Clients, services: Vec<Service>) {
        let mut futures = Vec::new();

        for service in services {
            // Save the service to the primary instance and get the URL for on-chain
            let service_url = DeployService::save_service(&clients.cli_ctx, &service)
                .await
                .unwrap();

            let service_manager_instance = self.lookup.get(&service.name).unwrap();
            let http_clients = clients.http_clients.clone();
            futures.push(async move {
                match service_manager_instance {
                    AnyServiceManagerInstance::Evm { manager, .. } => {
                        // wait for the trigger streams to be ready on all instances before we update the service uri
                        for (idx, http_client) in http_clients.iter().enumerate() {
                            tracing::info!(
                                "Waiting for trigger streams on instance {} for service {}",
                                idx,
                                service.name
                            );
                            wait_for_evm_trigger_streams_to_finalize(
                                http_client,
                                Some(service.manager.clone()),
                            )
                            .await;
                        }
                        manager.set_service_uri(service_url).await.unwrap();
                    }
                    AnyServiceManagerInstance::Cosmos { manager, .. } => {
                        manager.set_service_uri(&service_url).await.unwrap();
                    }
                }

                // Directly add the service to ALL instances using the dev endpoint
                // This bypasses on-chain event detection which may not work reliably
                // across multiple WAVS instances in the test environment
                for (idx, http_client) in http_clients.iter().enumerate() {
                    tracing::info!(
                        "Directly adding service to instance {} for service {}",
                        idx,
                        service.name
                    );
                    // Ignore "already registered" errors - the instance may have
                    // already received the service via on-chain event detection
                    match http_client.dev_add_service_direct(&service).await {
                        Ok(_) => {
                            tracing::info!(
                                "Service directly added to instance {} for service {}",
                                idx,
                                service.name
                            );
                        }
                        Err(e) if e.to_string().contains("already registered") => {
                            tracing::info!(
                                "Service already registered on instance {} for service {} (via on-chain detection)",
                                idx,
                                service.name
                            );
                        }
                        Err(e) => {
                            panic!(
                                "Failed to add service to instance {} for service {}: {}",
                                idx, service.name, e
                            );
                        }
                    }
                }

                // Wait for service update on all WAVS instances (should be instant now)
                for (idx, http_client) in http_clients.iter().enumerate() {
                    tracing::info!(
                        "Waiting for service update on instance {} for service {}",
                        idx,
                        service.name
                    );
                    http_client
                        .wait_for_service_update(&service, None)
                        .await
                        .unwrap();
                    tracing::info!(
                        "Service update complete on instance {} for service {}",
                        idx,
                        service.name
                    );
                }

                // doesn't hurt to wait again for rpcs at least in case trigger contract changed
                if let AnyServiceManagerInstance::Evm { .. } = service_manager_instance {
                    for (idx, http_client) in http_clients.iter().enumerate() {
                        tracing::info!(
                            "Final trigger stream wait on instance {} for service {}",
                            idx,
                            service.name
                        );
                        wait_for_evm_trigger_streams_to_finalize(http_client, None).await;
                    }
                }
            });
        }

        if self.configs.middleware_concurrency {
            let mut futures_unordered = FuturesUnordered::from_iter(futures);
            while (futures_unordered.next().await).is_some() {}
        } else {
            for future in futures {
                future.await;
            }
        }
    }
}
