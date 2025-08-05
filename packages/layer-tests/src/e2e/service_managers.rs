use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use futures::{stream::FuturesUnordered, StreamExt};
use utils::test_utils::{
    middleware::{AvsOperator, MiddlewareInstance, MiddlewareServiceManagerConfig},
    mock_service_manager::MockServiceManager,
};
use wavs_cli::command::deploy_service::{DeployService, DeployServiceArgs};
use wavs_types::{
    ChainName, Service, ServiceID, ServiceManager, ServiceStatus, SigningKeyResponse, Submit,
};

use crate::deployment::ServiceDeployment;

use crate::e2e::{
    clients::Clients,
    components::ComponentSources,
    config::Configs,
    helpers::create_service_for_test,
    test_registry::{CosmosTriggerCodeMap, TestRegistry},
};

#[derive(Clone)]
pub struct ServiceManagers {
    configs: Arc<Configs>,
    lookup: Arc<HashMap<String, (MockServiceManager, ChainName)>>,
    aggregator_registered_service_ids: Arc<std::sync::Mutex<HashSet<ServiceID>>>,
}

impl ServiceManagers {
    pub fn new(configs: Configs) -> Self {
        Self {
            lookup: Arc::new(HashMap::new()),
            aggregator_registered_service_ids: Arc::new(std::sync::Mutex::new(HashSet::new())),
            configs: Arc::new(configs),
        }
    }
}

impl ServiceManagers {
    pub async fn bootstrap(
        &mut self,
        registry: &TestRegistry,
        clients: &Clients,
        middleware_instance: MiddlewareInstance,
    ) {
        tracing::warn!("WAVS Concurrency: {}", self.configs.wavs_concurrency);
        tracing::warn!(
            "Middleware Concurrency: {}",
            self.configs.middleware_concurrency
        );
        tracing::warn!("Bootstrapping service managers...");
        self.deploy_service_managers(registry, clients, middleware_instance)
            .await;
        tracing::warn!("Bootstrapping initial service uris...");
        self.set_initial_service_uris(registry, clients).await;
        tracing::warn!("Bootstrapping initial services...");
        self.deploy_initial_wavs_services(registry, clients).await;
        tracing::warn!("Bootstrapping register operators...");
        self.register_operators(registry, clients).await;
    }

    pub fn get_service_manager(&self, test_name: &str) -> ServiceManager {
        let (mock_service_manager, chain_name) = self.lookup.get(test_name).unwrap();
        ServiceManager::Evm {
            chain_name: chain_name.clone(),
            address: mock_service_manager.address(),
        }
    }

    pub async fn deploy_service_managers(
        &mut self,
        registry: &TestRegistry,
        clients: &Clients,
        middleware_instance: MiddlewareInstance,
    ) {
        let mut lookup = HashMap::new();

        let mut futures = Vec::new();

        for test in registry.list_all() {
            let chain_name = test.service_manager_chain.clone();
            let wallet_client = clients.get_evm_client(&chain_name);
            let test_name = test.name.clone();
            let middleware_instance = middleware_instance.clone();
            futures.push(async move {
                tracing::info!("Deploying service manager for test {}", test_name);
                let service_manager = MockServiceManager::new(middleware_instance, wallet_client)
                    .await
                    .unwrap();
                tracing::info!(
                    "Service manager for test {} is {}",
                    test_name,
                    service_manager.address()
                );
                (test_name, (service_manager, chain_name))
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
            let (mock_service_manager, chain_name) = self.lookup.get(&test.name).unwrap();
            let service_manager = ServiceManager::Evm {
                chain_name: chain_name.clone(),
                address: mock_service_manager.address(),
            };

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

            futures.push(async move {
                mock_service_manager
                    .set_service_uri(service_url)
                    .await
                    .unwrap();
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

            futures.push(async move {
                tracing::info!("Deploying service {} on WAVS", test.name);

                // Deploy the service on WAVS
                DeployService::run(
                    &clients.cli_ctx,
                    DeployServiceArgs {
                        service_manager,
                        set_service_url_args: None,
                    },
                )
                .await
                .unwrap();
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

    pub async fn register_operators(&self, registry: &TestRegistry, clients: &Clients) {
        let mut futures = Vec::new();

        for test in registry.list_all() {
            let (mock_service_manager, chain_name) = self.lookup.get(&test.name).unwrap();
            let service_manager = ServiceManager::Evm {
                chain_name: chain_name.clone(),
                address: mock_service_manager.address(),
            };

            let SigningKeyResponse::Secp256k1 {
                evm_address: avs_signer_address,
                ..
            } = clients
                .http_client
                .get_service_key(service_manager.clone())
                .await
                .unwrap();

            let operator_address = clients
                .get_evm_client(&test.service_manager_chain)
                .address();
            let avs_operator =
                AvsOperator::new(operator_address, avs_signer_address.parse().unwrap());

            futures.push(async move {
                mock_service_manager
                    .configure(&MiddlewareServiceManagerConfig::new(&[avs_operator], 1))
                    .await
                    .unwrap();
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
        cosmos_trigger_code_map: CosmosTriggerCodeMap,
    ) -> HashMap<String, ServiceDeployment> {
        let mut futures = Vec::new();

        for test in registry.list_all() {
            let service_manager = self.get_service_manager(&test.name);

            futures.push(create_service_for_test(
                test,
                clients,
                component_sources,
                service_manager,
                cosmos_trigger_code_map.clone(),
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
            let (mock_service_manager, _) = self.lookup.get(&service.name).unwrap();

            // register the service to the aggregator if needed
            for workflow in service.workflows.values() {
                if self
                    .aggregator_registered_service_ids
                    .lock()
                    .unwrap()
                    .insert(service.id())
                {
                    if let Submit::Aggregator { url, .. } = &workflow.submit {
                        TestRegistry::register_to_aggregator(url, &service)
                            .await
                            .unwrap();
                    }
                }
            }

            let service_url = DeployService::save_service(&clients.cli_ctx, &service)
                .await
                .unwrap();

            futures.push(async move {
                mock_service_manager
                    .set_service_uri(service_url)
                    .await
                    .unwrap();
                clients
                    .http_client
                    .wait_for_service_update(&service, None)
                    .await
                    .unwrap();
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
