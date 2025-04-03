use std::collections::BTreeMap;

use crate::{
    e2e::digests::DigestName,
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_eth_client::{
        example_service_manager::SimpleServiceManager, example_trigger::SimpleTrigger,
    },
};

use super::{
    clients::Clients,
    config::Configs,
    digests::Digests,
    matrix::{AnyService, CosmosService, CrossChainService, EthService},
};
use crate::example_eth_client::{example_submit::SimpleSubmit, SimpleEthTriggerClient};
use alloy::sol_types::SolEvent;
use utils::{context::AppContext, filesystem::workspace_path};
use wavs_cli::{
    args::{CliSubmitKind, CliTriggerKind},
    command::deploy_service_raw::{DeployServiceRaw, DeployServiceRawArgs},
};
use wavs_types::{
    AllowedHostPermission, ByteArray, ChainName, Component, ComponentID, ComponentSource,
    EthereumContractSubmission, Permissions, Service, ServiceConfig, ServiceID, ServiceManager,
    ServiceStatus, Submit, Trigger, Workflow, WorkflowID,
};

#[derive(Default)]
pub struct Services {
    pub lookup: BTreeMap<AnyService, Vec<Service>>,
}

impl Services {
    pub fn new(ctx: AppContext, configs: &Configs, clients: &Clients, digests: &Digests) -> Self {
        ctx.rt.block_on(async move {
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

            let mut eth_service_managers = BTreeMap::default();
            // hrmf, "nonce too low" errors, gotta go sequentially...

            for chain in chain_names
                .eth
                .iter()
                .chain(chain_names.eth_aggregator.iter())
            {
                let eth_client = clients.cli_ctx.get_eth_client(chain).unwrap();

                let service_manager = SimpleServiceManager::deploy(eth_client.provider)
                    .await
                    .unwrap();

                eth_service_managers.insert(chain.clone(), *service_manager.address());
            }

            let all_services = configs
                .matrix
                .eth
                .iter()
                .map(|s| (*s).into())
                .chain(configs.matrix.cosmos.iter().map(|s| (*s).into()))
                .chain(configs.matrix.cross_chain.iter().map(|s| (*s).into()))
                .collect::<Vec<AnyService>>();

            let mut lookup = BTreeMap::default();
            let mut cosmos_code_ids = BTreeMap::default();

            // nonce errors here too, gotta go sequentially :/
            for service_kind in all_services {
                let service = match service_kind {
                    AnyService::Eth(EthService::MultiWorkflow) => {
                        deploy_service_raw(
                            service_kind,
                            clients,
                            digests,
                            &chain_names,
                            &eth_service_managers,
                        )
                        .await
                    }
                    _ => {
                        deploy_service_simple(
                            service_kind,
                            configs,
                            clients,
                            digests,
                            &chain_names,
                            &mut cosmos_code_ids,
                            &eth_service_managers,
                        )
                        .await
                    }
                };

                if service_kind == AnyService::Eth(EthService::MultiTrigger) {
                    let mut additional_service = service.clone();

                    additional_service.id =
                        ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap();

                    DeployServiceRaw::run(
                        &clients.cli_ctx,
                        DeployServiceRawArgs {
                            service: additional_service.clone(),
                        },
                    )
                    .await
                    .unwrap();

                    lookup.insert(service_kind, vec![service, additional_service]);
                } else {
                    lookup.insert(service_kind, vec![service]);
                }
            }

            Self { lookup }
        })
    }
}

async fn deploy_service_simple(
    service_kind: AnyService,
    _configs: &Configs,
    clients: &Clients,
    digests: &Digests,
    chain_names: &ChainNames,
    cosmos_code_ids: &mut BTreeMap<ChainName, u64>,
    eth_service_managers: &BTreeMap<ChainName, alloy::primitives::Address>,
) -> Service {
    let digest_name = Vec::<DigestName>::from(service_kind)[0];
    let digest = digests.lookup.get(&digest_name).unwrap().clone();

    let trigger = match service_kind {
        AnyService::Eth(EthService::BlockInterval) => CliTriggerKind::EthBlockInterval,
        AnyService::Eth(EthService::CronInterval) => CliTriggerKind::CronInterval,
        AnyService::Eth(_) => CliTriggerKind::EthContractEvent,
        AnyService::Cosmos(CosmosService::CronInterval) => CliTriggerKind::CronInterval,
        AnyService::Cosmos(CosmosService::BlockInterval) => CliTriggerKind::CosmosBlockInterval,
        AnyService::Cosmos(_) => CliTriggerKind::CosmosContractEvent,
        AnyService::CrossChain(service) => match service {
            CrossChainService::CosmosToEthEchoData => CliTriggerKind::CosmosContractEvent,
        },
    };

    let trigger_chain = match trigger {
        CliTriggerKind::EthContractEvent => match service_kind {
            AnyService::Eth(EthService::EchoDataAggregator) => {
                Some(chain_names.eth_aggregator[0].clone())
            }
            AnyService::Eth(EthService::EchoDataSecondaryChain) => Some(chain_names.eth[1].clone()),
            _ => Some(chain_names.eth[0].clone()),
        },
        CliTriggerKind::CosmosContractEvent => Some(chain_names.cosmos[0].clone()),
        CliTriggerKind::EthBlockInterval => Some(chain_names.eth[0].clone()),
        CliTriggerKind::CosmosBlockInterval => Some(chain_names.cosmos[0].clone()),
        CliTriggerKind::CronInterval => None,
    };

    let submit = match service_kind {
        _ => CliSubmitKind::EthServiceHandler,
    };

    let submit_chain = match submit {
        CliSubmitKind::EthServiceHandler => match trigger {
            CliTriggerKind::EthContractEvent => trigger_chain.clone(), // not strictly necessary, just easier to reason about same-chain
            CliTriggerKind::CosmosContractEvent => Some(chain_names.eth[0].clone()), // always eth for now
            CliTriggerKind::EthBlockInterval => trigger_chain.clone(),
            CliTriggerKind::CosmosBlockInterval => Some(chain_names.eth[0].clone()), // always eth for now as above
            CliTriggerKind::CronInterval => Some(chain_names.eth[0].clone()),
        },
        CliSubmitKind::None => None,
    };

    // Create the actual trigger based on the trigger kind
    let actual_trigger = match trigger {
        CliTriggerKind::EthContractEvent => {
            if let Some(chain_name) = &trigger_chain {
                let client = clients.cli_ctx.get_eth_client(chain_name).unwrap();

                tracing::info!("Deploying new eth trigger contract");
                let address = *SimpleTrigger::deploy(client.provider)
                    .await
                    .unwrap()
                    .address();

                let event_hash =
                    *crate::example_eth_client::example_trigger::NewTrigger::SIGNATURE_HASH;

                Trigger::EthContractEvent {
                    chain_name: chain_name.clone(),
                    address,
                    event_hash: ByteArray::new(event_hash),
                }
            } else {
                panic!("Chain name required for EthContractEvent");
            }
        }
        CliTriggerKind::CosmosContractEvent => {
            if let Some(chain_name) = &trigger_chain {
                let client = clients.cli_ctx.get_cosmos_client(chain_name).unwrap();

                let code_id = match cosmos_code_ids.get(chain_name).cloned() {
                    Some(code_id) => code_id,
                    None => {
                        let path_to_wasm = workspace_path()
                            .join("examples")
                            .join("build")
                            .join("contracts")
                            .join("simple_example.wasm");

                        let wasm_byte_code = std::fs::read(path_to_wasm).unwrap();

                        let (code_id, _) = client
                            .contract_upload_file(wasm_byte_code, None)
                            .await
                            .unwrap();

                        cosmos_code_ids.insert(chain_name.clone(), code_id);

                        code_id
                    }
                };

                let contract_address = SimpleCosmosTriggerClient::new_code_id(client, code_id)
                    .await
                    .unwrap()
                    .contract_address;

                Trigger::CosmosContractEvent {
                    chain_name: chain_name.clone(),
                    address: contract_address,
                    event_type: crate::example_cosmos_client::NewMessageEvent::KEY.to_string(),
                }
            } else {
                panic!("Chain name required for CosmosContractEvent");
            }
        }
        CliTriggerKind::EthBlockInterval => {
            if let Some(chain_name) = &trigger_chain {
                Trigger::BlockInterval {
                    chain_name: chain_name.clone(),
                    n_blocks: std::num::NonZeroU32::new(1).unwrap(),
                }
            } else {
                panic!("Chain name required for EthBlockInterval");
            }
        }
        CliTriggerKind::CosmosBlockInterval => {
            if let Some(chain_name) = &trigger_chain {
                Trigger::BlockInterval {
                    chain_name: chain_name.clone(),
                    n_blocks: std::num::NonZeroU32::new(1).unwrap(),
                }
            } else {
                panic!("Chain name required for CosmosBlockInterval");
            }
        }
        CliTriggerKind::CronInterval => Trigger::Cron {
            schedule: "* * * * * *".to_string(),
            start_time: None,
            end_time: None,
        },
    };

    // Create the actual submit based on the submit kind and chain
    let actual_submit = match submit {
        CliSubmitKind::EthServiceHandler => {
            if let Some(chain) = &submit_chain {
                let client = clients.cli_ctx.get_eth_client(chain).unwrap();
                let service_manager_address = *eth_service_managers.get(chain).unwrap();

                tracing::info!("Deploying new eth submit contract");
                let submit_address =
                    *SimpleSubmit::deploy(client.provider, service_manager_address)
                        .await
                        .unwrap()
                        .address();

                Submit::EthereumContract(EthereumContractSubmission {
                    chain_name: chain.clone(),
                    address: submit_address,
                    max_gas: None,
                })
            } else {
                // Should not happen with EthServiceHandler, but just in case
                Submit::None
            }
        }
        CliSubmitKind::None => Submit::None,
    };

    // Create Component
    let component_id = ComponentID::new("default").unwrap();
    let workflow_id = WorkflowID::new("default").unwrap();

    let component = Component {
        source: ComponentSource::Digest(digest),
        permissions: Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        },
    };

    // Create Workflow
    let workflow = Workflow {
        trigger: actual_trigger,
        component: component_id.clone(),
        submit: actual_submit,
        fuel_limit: None,
        aggregator: None,
    };

    // Get service manager address from the submit chain
    let service_manager_chain = match &submit_chain {
        Some(chain) => chain.clone(),
        None => chain_names.eth[0].clone(),
    };
    let service_manager_address = *eth_service_managers.get(&service_manager_chain).unwrap();

    // Create Service
    let service = Service {
        id: ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap(),
        name: format!("{:?}", service_kind),
        components: BTreeMap::from([(component_id, component)]),
        workflows: BTreeMap::from([(workflow_id, workflow)]),
        status: ServiceStatus::Active,
        config: ServiceConfig::default(),
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
    DeployServiceRaw::run(
        &clients.cli_ctx,
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
    digests: &Digests,
    chain_names: &ChainNames,
    eth_service_managers: &BTreeMap<ChainName, alloy::primitives::Address>,
) -> Service {
    if !matches!(service_kind, AnyService::Eth(EthService::MultiWorkflow)) {
        panic!("unexpected service kind: {:?}", service_kind);
    }

    let trigger1 = deploy_trigger_raw(clients, chain_names).await;
    let trigger2 = deploy_trigger_raw(clients, chain_names).await;

    let component_id1 = ComponentID::new("component1").unwrap();
    let component_id2 = ComponentID::new("component2").unwrap();

    let digest_names = Vec::<DigestName>::from(service_kind);

    let component1 = Component {
        source: ComponentSource::Digest(digests.lookup.get(&digest_names[0]).unwrap().clone()),
        permissions: Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        },
    };

    let component2 = Component {
        source: ComponentSource::Digest(digests.lookup.get(&digest_names[1]).unwrap().clone()),
        permissions: Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        },
    };

    let submit1 = deploy_submit_raw(clients, chain_names, eth_service_managers).await;
    let submit2 = deploy_submit_raw(clients, chain_names, eth_service_managers).await;

    let workflow_id1 = WorkflowID::new("workflow1").unwrap();
    let workflow_id2 = WorkflowID::new("workflow2").unwrap();

    let workflow1 = Workflow {
        trigger: trigger1,
        component: component_id1,
        submit: submit1,
        fuel_limit: None,
        aggregator: None,
    };

    let workflow2 = Workflow {
        trigger: trigger2,
        component: component_id2,
        submit: submit2,
        fuel_limit: None,
        aggregator: None,
    };

    let components = BTreeMap::from([
        (workflow1.component.clone(), component1),
        (workflow2.component.clone(), component2),
    ]);

    let workflows = BTreeMap::from([(workflow_id1, workflow1), (workflow_id2, workflow2)]);

    let service_manager_address = *eth_service_managers.get(&chain_names.eth[0]).unwrap();

    let service = Service {
        id: ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap(),
        name: "".to_string(),
        components,
        workflows,
        status: ServiceStatus::Active,
        config: ServiceConfig::default(),
        manager: ServiceManager::Ethereum {
            chain_name: chain_names.eth[0].clone(),
            address: service_manager_address,
        },
    };

    DeployServiceRaw::run(
        &clients.cli_ctx,
        DeployServiceRawArgs {
            service: service.clone(),
        },
    )
    .await
    .unwrap();

    service
}

async fn deploy_trigger_raw(clients: &Clients, chain_names: &ChainNames) -> Trigger {
    let chain_name = chain_names.eth[0].clone();
    let eth_client = clients.cli_ctx.get_eth_client(&chain_name).unwrap().clone();
    let event_hash = *crate::example_eth_client::example_trigger::NewTrigger::SIGNATURE_HASH;

    let address = SimpleEthTriggerClient::deploy(eth_client.provider)
        .await
        .unwrap();

    Trigger::EthContractEvent {
        chain_name,
        address,
        event_hash: ByteArray::new(event_hash),
    }
}

async fn deploy_submit_raw(
    clients: &Clients,
    chain_names: &ChainNames,
    eth_service_managers: &BTreeMap<ChainName, alloy::primitives::Address>,
) -> Submit {
    let chain_name = chain_names.eth[0].clone();
    let eth_client = clients.cli_ctx.get_eth_client(&chain_name).unwrap().clone();

    let service_manager_address = *eth_service_managers.get(&chain_name).unwrap();

    let simple_submit = SimpleSubmit::deploy(eth_client.provider, service_manager_address)
        .await
        .unwrap();

    Submit::EthereumContract(EthereumContractSubmission {
        chain_name,
        address: *simple_submit.address(),
        max_gas: None,
    })
}

#[derive(Debug, Default)]
struct ChainNames {
    eth: Vec<ChainName>,
    eth_aggregator: Vec<ChainName>,
    cosmos: Vec<ChainName>,
}
