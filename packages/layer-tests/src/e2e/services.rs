use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use crate::{
    e2e::components::ComponentName,
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_eth_client::{
        example_service_manager::SimpleServiceManager, example_trigger::SimpleTrigger,
    },
};

use super::{
    clients::Clients,
    components::ComponentSources,
    config::Configs,
    matrix::{AnyService, CosmosService, EthService},
};
use crate::example_eth_client::{example_submit::SimpleSubmit, SimpleEthTriggerClient};
use alloy::{primitives::Address, providers::ext::AnvilApi, sol_types::SolEvent};
use futures::{stream::FuturesUnordered, StreamExt};
use utils::{context::AppContext, filesystem::workspace_path};
use wavs_cli::command::deploy_service_raw::{DeployServiceRaw, DeployServiceRawArgs};
use wavs_types::{
    AllowedHostPermission, ByteArray, ChainName, Component, EthereumContractSubmission,
    Permissions, Service, ServiceID, ServiceManager, ServiceStatus, Submit, Trigger, Workflow,
    WorkflowID,
};

#[derive(Default)]
pub struct Services {
    // second service is for multi-trigger tests only
    pub lookup: BTreeMap<AnyService, (Service, Option<Service>)>,
}

impl Services {
    pub fn new(
        ctx: AppContext,
        configs: &Configs,
        clients: &Clients,
        component_sources: &ComponentSources,
    ) -> Self {
        let mut chain_names = ChainNames::default();

        for (chain_name, chain) in configs.chains.eth.iter() {
            if chain.aggregator_endpoint.is_some() {
                chain_names.eth_aggregator.push(chain_name.clone());
            } else {
                chain_names.eth.push(chain_name.clone());
            }
        }

        chain_names.cosmos = configs.chains.cosmos.keys().cloned().collect::<Vec<_>>();

        tracing::info!("chain names: {:?}", chain_names);

        // the ethereum bytecode is all bundled in via sol macro
        // but for cosmos, we need to read it in.
        // lets keep the heavy io out of async tasks
        // and predeploy the code_ids so they're ready
        let cosmos_code_id = if configs.matrix.cosmos_regular_chain_enabled() {
            let cosmos_bytecode = std::fs::read(
                workspace_path()
                    .join("examples")
                    .join("build")
                    .join("contracts")
                    .join("simple_example.wasm"),
            )
            .unwrap();

            let chain_name = chain_names.cosmos[0].clone();
            let client = clients.get_cosmos_client(&chain_name);

            tracing::info!("Deploying new cosmos trigger contract");

            tracing::info!(
                "Uploading cosmos wasm byte code ({} bytes) to chain {}",
                cosmos_bytecode.len(),
                chain_name
            );

            let (code_id, _) = ctx.rt.block_on({
                let cosmos_bytecode = cosmos_bytecode.clone();
                async move {
                    client
                        .contract_upload_file(cosmos_bytecode, None)
                        .await
                        .unwrap()
                }
            });

            tracing::info!("Uploaded wasm byte code to chain {}", chain_name,);

            Some(code_id)
        } else {
            None
        };

        let all_services = configs
            .matrix
            .eth
            .iter()
            .map(|s| (*s).into())
            .chain(configs.matrix.cosmos.iter().map(|s| (*s).into()))
            .chain(configs.matrix.cross_chain.iter().map(|s| (*s).into()))
            .collect::<Vec<AnyService>>();

        let lookup = Arc::new(Mutex::new(BTreeMap::default()));

        let mut serial_futures = Vec::new();
        let mut concurrent_futures = FuturesUnordered::new();

        for service_kind in all_services {
            let lookup = lookup.clone();
            let cosmos_code_id = cosmos_code_id.clone();
            let chain_names = chain_names.clone();

            let fut = async move {
                let service = match service_kind {
                    AnyService::Eth(EthService::MultiWorkflow) => {
                        deploy_service_raw(service_kind, clients, component_sources, &chain_names)
                            .await
                    }
                    _ => {
                        deploy_service_simple(
                            service_kind,
                            configs,
                            clients,
                            component_sources,
                            &chain_names,
                            cosmos_code_id.clone(),
                        )
                        .await
                    }
                };

                clients
                    .get_eth_client(service.manager.chain_name())
                    .provider
                    .evm_mine(None)
                    .await
                    .unwrap();

                tracing::info!("[{:?}] Deployed service: {}", service_kind, service.id);

                if service_kind == AnyService::Eth(EthService::MultiTrigger) {
                    // it's a bit ugly but it works, just clone the original service and replace:
                    // 1. the service id (so it's a new service, from the perspective of WAVS)
                    // 2. the workflow submission
                    //
                    // ultimately this means the trigger from the original service
                    // should cause this service to submit too - albeit to a different service handler
                    let mut additional_service = service.clone();

                    additional_service.id =
                        ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap();

                    let service_manager_address =
                        deploy_service_manager(clients, service.manager.chain_name()).await;

                    for (_, workflow) in additional_service.workflows.iter_mut() {
                        workflow.submit = deploy_submit(
                            clients,
                            service.manager.chain_name(),
                            service_manager_address,
                        )
                        .await;
                    }

                    // now we've patched it - just call the CLI command directly
                    DeployServiceRaw::run(
                        &clients.cli_ctx,
                        clients
                            .get_eth_client(service.manager.chain_name())
                            .provider
                            .clone(),
                        DeployServiceRawArgs {
                            service: additional_service.clone(),
                        },
                    )
                    .await
                    .unwrap();

                    tracing::info!(
                        "[{:?}] Deployed service #2: {}",
                        service_kind,
                        additional_service.id
                    );

                    lookup
                        .lock()
                        .unwrap()
                        .insert(service_kind, (service, Some(additional_service)));
                } else {
                    lookup.lock().unwrap().insert(service_kind, (service, None));
                }
            };

            if service_kind.concurrent() {
                concurrent_futures.push(fut);
            } else {
                serial_futures.push(fut);
            }
        }

        ctx.rt.block_on(async move {
            tracing::info!("\n\n Deploying serial services...");
            for fut in serial_futures {
                fut.await;
            }

            tracing::info!("\n\n Deploying concurrent services...");
            while let Some(_) = concurrent_futures.next().await {}
        });

        let lookup = {
            let lock = lookup.lock().unwrap();
            lock.clone()
        };

        Self { lookup }
    }
}

async fn deploy_service_simple(
    service_kind: AnyService,
    _configs: &Configs,
    clients: &Clients,
    component_sources: &ComponentSources,
    chain_names: &ChainNames,
    cosmos_code_id: Option<u64>,
) -> Service {
    let component_name = Vec::<ComponentName>::from(service_kind)[0];
    let component_source = component_sources
        .lookup
        .get(&component_name)
        .unwrap()
        .clone();

    // Determine trigger chain directly based on service_kind
    let trigger_chain = match service_kind {
        AnyService::Eth(EthService::EchoDataAggregator) => {
            Some(chain_names.eth_aggregator[0].clone())
        }
        AnyService::Eth(EthService::EchoDataSecondaryChain) => Some(chain_names.eth[1].clone()),
        AnyService::Eth(_) => Some(chain_names.eth[0].clone()),
        AnyService::Cosmos(_) => Some(chain_names.cosmos[0].clone()),
        AnyService::CrossChain(_) => Some(chain_names.cosmos[0].clone()),
    };

    // Create the actual trigger based on the service_kind
    let trigger = match service_kind {
        AnyService::Eth(EthService::BlockInterval) => {
            let chain_name = trigger_chain.as_ref().unwrap().clone();
            Trigger::BlockInterval {
                chain_name,
                n_blocks: std::num::NonZeroU32::new(1).unwrap(),
            }
        }
        AnyService::Eth(EthService::CronInterval) => Trigger::Cron {
            schedule: "*/1 * * * * *".to_string(),
            start_time: None,
            end_time: None,
        },
        AnyService::Eth(_) => {
            let chain_name = trigger_chain.as_ref().unwrap().clone();
            let client = clients.get_eth_client(trigger_chain.as_ref().unwrap());

            tracing::info!("[{:?}] Deploying new eth trigger contract", service_kind);
            let address = *SimpleTrigger::deploy(client.provider.clone())
                .await
                .unwrap()
                .address();

            let event_hash =
                *crate::example_eth_client::example_trigger::NewTrigger::SIGNATURE_HASH;

            Trigger::EthContractEvent {
                chain_name,
                address,
                event_hash: ByteArray::new(event_hash),
            }
        }
        AnyService::Cosmos(CosmosService::CronInterval) => Trigger::Cron {
            schedule: "*/10 * * * * *".to_string(),
            start_time: None,
            end_time: None,
        },
        AnyService::Cosmos(CosmosService::BlockInterval) => {
            let chain_name = trigger_chain.as_ref().unwrap().clone();
            Trigger::BlockInterval {
                chain_name,
                n_blocks: std::num::NonZeroU32::new(1).unwrap(),
            }
        }
        AnyService::Cosmos(_) | AnyService::CrossChain(_) => {
            let code_id = cosmos_code_id.unwrap();

            tracing::info!(
                "[{:?}] Deploying new cosmos trigger contract (code id: {})",
                service_kind,
                code_id
            );

            let chain_name = trigger_chain.as_ref().unwrap().clone();
            let client = clients.get_cosmos_client(&chain_name);

            let contract_address = SimpleCosmosTriggerClient::new_code_id(client, code_id)
                .await
                .unwrap()
                .contract_address;

            tracing::info!(
                "[{:?}] Deployed new cosmos trigger contract (address: {})",
                service_kind,
                contract_address
            );

            Trigger::CosmosContractEvent {
                chain_name,
                address: contract_address,
                event_type: crate::example_cosmos_client::NewMessageEvent::KEY.to_string(),
            }
        }
    };

    // Determine the submit chain
    let submit_chain = match service_kind {
        AnyService::Eth(_) => trigger_chain.clone(),
        AnyService::Cosmos(_) | AnyService::CrossChain(_) => Some(chain_names.eth[0].clone()),
    };

    // Get service manager address from the submit chain
    let service_manager_chain = match &submit_chain {
        Some(chain) => chain.clone(),
        None => chain_names.eth[0].clone(),
    };
    let service_manager_address = deploy_service_manager(clients, &service_manager_chain).await;

    // Create the actual submit
    let submit = if let Some(chain) = &submit_chain {
        let client = clients.get_eth_client(chain);

        tracing::info!("[{:?}] Deploying new eth submit contract", service_kind);
        let address = *SimpleSubmit::deploy(client.provider.clone(), service_manager_address)
            .await
            .unwrap()
            .address();

        tracing::info!(
            "[{:?}] Deployed new eth submit contract: {}",
            service_kind,
            address
        );

        Submit::EthereumContract(EthereumContractSubmission {
            chain_name: chain.clone(),
            address,
            max_gas: None,
        })
    } else {
        Submit::None
    };

    // Create Component
    let workflow_id = WorkflowID::new("default").unwrap();

    let mut component = Component::new(component_source);
    component.permissions = Permissions {
        allowed_http_hosts: AllowedHostPermission::All,
        file_system: true,
    };

    // Create Workflow
    let workflow = Workflow {
        trigger,
        component,
        submit,
        aggregator: None,
    };

    // Create Service
    let service = Service {
        id: ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap(),
        name: format!("{:?}", service_kind),
        workflows: BTreeMap::from([(workflow_id, workflow)]),
        status: ServiceStatus::Active,
        manager: ServiceManager::Ethereum {
            chain_name: service_manager_chain,
            address: service_manager_address,
        },
    };

    tracing::info!(
        "Deploying Service {} on trigger_chain: [{}] submit_chain: [{}]",
        match service_kind {
            AnyService::Eth(service) => format!("Ethereum {:?}", service),
            AnyService::Cosmos(service) => format!("Cosmos {:?}", service),
            AnyService::CrossChain(service) => format!("CrossChain {:?}", service),
        },
        trigger_chain.as_deref().unwrap_or("none"),
        submit_chain.as_deref().unwrap_or("none")
    );

    // Deploy using DeployServiceRaw instead of DeployService

    let submit_client = clients.get_eth_client(service.manager.chain_name());
    DeployServiceRaw::run(
        &clients.cli_ctx,
        submit_client.provider.clone(),
        DeployServiceRawArgs {
            service: service.clone(),
        },
    )
    .await
    .unwrap();

    service
}

async fn deploy_service_raw(
    service_kind: AnyService,
    clients: &Clients,
    component_sources: &ComponentSources,
    chain_names: &ChainNames,
) -> Service {
    if !matches!(service_kind, AnyService::Eth(EthService::MultiWorkflow)) {
        panic!("unexpected service kind: {:?}", service_kind);
    }

    let trigger1 = deploy_trigger(clients, chain_names).await;
    let trigger2 = deploy_trigger(clients, chain_names).await;

    let component_names = Vec::<ComponentName>::from(service_kind);

    let mut component1 = Component::new(
        component_sources
            .lookup
            .get(&component_names[0])
            .unwrap()
            .clone(),
    );

    component1.permissions = Permissions {
        allowed_http_hosts: AllowedHostPermission::All,
        file_system: true,
    };

    let mut component2 = Component::new(
        component_sources
            .lookup
            .get(&component_names[1])
            .unwrap()
            .clone(),
    );

    component2.permissions = Permissions {
        allowed_http_hosts: AllowedHostPermission::All,
        file_system: true,
    };

    let chain_name = chain_names.eth[0].clone();
    let service_manager_address = deploy_service_manager(clients, &chain_name).await;

    let submit1 = deploy_submit(clients, &chain_name, service_manager_address).await;
    let submit2 = deploy_submit(clients, &chain_name, service_manager_address).await;

    let workflow_id1 = WorkflowID::new("workflow1").unwrap();
    let workflow_id2 = WorkflowID::new("workflow2").unwrap();

    let workflow1 = Workflow {
        trigger: trigger1,
        component: component1,
        submit: submit1,
        aggregator: None,
    };

    let workflow2 = Workflow {
        trigger: trigger2,
        component: component2,
        submit: submit2,
        aggregator: None,
    };

    let workflows = BTreeMap::from([(workflow_id1, workflow1), (workflow_id2, workflow2)]);

    let service = Service {
        id: ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap(),
        name: "".to_string(),
        workflows,
        status: ServiceStatus::Active,
        manager: ServiceManager::Ethereum {
            chain_name,
            address: service_manager_address,
        },
    };

    DeployServiceRaw::run(
        &clients.cli_ctx,
        clients
            .get_eth_client(service.manager.chain_name())
            .provider
            .clone(),
        DeployServiceRawArgs {
            service: service.clone(),
        },
    )
    .await
    .unwrap();

    service
}

async fn deploy_trigger(clients: &Clients, chain_names: &ChainNames) -> Trigger {
    let chain_name = chain_names.eth[0].clone();
    let client = clients.get_eth_client(&chain_name);
    let event_hash = *crate::example_eth_client::example_trigger::NewTrigger::SIGNATURE_HASH;

    let address = SimpleEthTriggerClient::deploy(client.provider.clone())
        .await
        .unwrap();

    Trigger::EthContractEvent {
        chain_name,
        address,
        event_hash: ByteArray::new(event_hash),
    }
}

async fn deploy_submit(
    clients: &Clients,
    chain_name: &ChainName,
    service_manager_address: Address,
) -> Submit {
    let eth_client = clients.get_eth_client(chain_name);

    let address = *SimpleSubmit::deploy(eth_client.provider.clone(), service_manager_address)
        .await
        .unwrap()
        .address();

    Submit::EthereumContract(EthereumContractSubmission {
        chain_name: chain_name.clone(),
        address,
        max_gas: None,
    })
}

async fn deploy_service_manager(clients: &Clients, chain_name: &ChainName) -> Address {
    let eth_client = clients.get_eth_client(chain_name);

    *SimpleServiceManager::deploy(eth_client.provider.clone())
        .await
        .unwrap()
        .address()
}

#[derive(Debug, Default, Clone)]
struct ChainNames {
    eth: Vec<ChainName>,
    eth_aggregator: Vec<ChainName>,
    cosmos: Vec<ChainName>,
}
