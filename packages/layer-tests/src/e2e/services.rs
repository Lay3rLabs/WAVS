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
    command::{
        deploy_service::{DeployService, DeployServiceArgs},
        deploy_service_raw::{DeployServiceRaw, DeployServiceRawArgs},
    },
};
use wavs_types::{
    AllowedHostPermission, ByteArray, ChainName, Component, ComponentID, ComponentSource,
    EthereumContractSubmission, Permissions, Service, ServiceConfig, ServiceID, ServiceManager,
    ServiceStatus, Submit, Trigger, Workflow, WorkflowID,
};

#[derive(Default)]
pub struct Services {
    // second service is for multi-trigger tests only
    pub lookup: BTreeMap<AnyService, (Service, Option<Service>)>,
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

                let service_manager = SimpleServiceManager::deploy(eth_client.provider.clone())
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
                    // it's a bit ugly but it works, just clone the original service and replace:
                    // 1. the service id (so it's a new service, from the perspective of WAVS)
                    // 2. the workflow submission
                    //
                    // ultimately this means the trigger from the original service
                    // should cause this service to submit too - albeit to a different service handler
                    let mut additional_service = service.clone();

                    additional_service.id =
                        ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap();

                    for (_, workflow) in additional_service.workflows.iter_mut() {
                        workflow.submit =
                            deploy_submit_raw(clients, &chain_names, &eth_service_managers).await;
                    }

                    // now we've patched it - just call the CLI command directly
                    DeployServiceRaw::run(
                        &clients.cli_ctx,
                        DeployServiceRawArgs {
                            service: additional_service.clone(),
                        },
                    )
                    .await
                    .unwrap();

                    lookup.insert(service_kind, (service, Some(additional_service)));
                } else {
                    lookup.insert(service_kind, (service, None));
                }
            }

            Self { lookup }
        })
    }
}

async fn deploy_service_simple(
    service: AnyService,
    _configs: &Configs,
    clients: &Clients,
    digests: &Digests,
    chain_names: &ChainNames,
    cosmos_code_ids: &mut BTreeMap<ChainName, u64>,
    eth_service_managers: &BTreeMap<ChainName, alloy::primitives::Address>,
) -> Service {
    let digest_name = Vec::<DigestName>::from(service)[0];
    let digest = digests.lookup.get(&digest_name).unwrap().clone();

    let trigger = match service {
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

    let trigger_event_name = match trigger {
        CliTriggerKind::EthContractEvent => Some(const_hex::encode(
            crate::example_eth_client::example_trigger::NewTrigger::SIGNATURE_HASH,
        )),
        CliTriggerKind::CosmosContractEvent => {
            Some(crate::example_cosmos_client::NewMessageEvent::KEY.to_string())
        }
        _ => None,
    };

    let submit = match service {
        _ => CliSubmitKind::EthServiceHandler,
    };

    let trigger_chain = match trigger {
        CliTriggerKind::EthContractEvent => match service {
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

    let trigger_address = match trigger {
        CliTriggerKind::EthContractEvent => {
            let client = clients
                .cli_ctx
                .get_eth_client(trigger_chain.as_ref().unwrap())
                .unwrap();

            tracing::info!("Deploying new eth trigger contract");
            Some(
                SimpleTrigger::deploy(client.provider)
                    .await
                    .unwrap()
                    .address()
                    .to_string(),
            )
        }
        CliTriggerKind::CosmosContractEvent => {
            let trigger_chain = trigger_chain.clone().unwrap();

            let client = clients.cli_ctx.get_cosmos_client(&trigger_chain).unwrap();

            let code_id = match cosmos_code_ids.get(&trigger_chain).cloned() {
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

                    cosmos_code_ids.insert(trigger_chain, code_id);

                    code_id
                }
            };

            Some(
                SimpleCosmosTriggerClient::new_code_id(client, code_id)
                    .await
                    .unwrap()
                    .contract_address
                    .to_string(),
            )
        }
        CliTriggerKind::EthBlockInterval => None,
        CliTriggerKind::CosmosBlockInterval => None,
        CliTriggerKind::CronInterval => None,
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

    let submit_address = match submit {
        CliSubmitKind::EthServiceHandler => {
            let submit_chain = submit_chain.as_ref().unwrap();
            let client = clients.cli_ctx.get_eth_client(submit_chain).unwrap();
            let service_manager_address = *eth_service_managers.get(submit_chain).unwrap();

            tracing::info!("Deploying new eth submit contract");
            Some(
                SimpleSubmit::deploy(client.provider.clone(), service_manager_address)
                    .await
                    .unwrap()
                    .address()
                    .to_string(),
            )
        }
        CliSubmitKind::None => None,
    };

    tracing::info!(
        "Deploying Service {} on trigger_chain: [{}] submit_chain: [{}]",
        match service {
            AnyService::Eth(service) => format!("Ethereum {:?}", service),
            AnyService::Cosmos(service) => format!("Cosmos {:?}", service),
            AnyService::CrossChain(service) => format!("CrossChain {:?}", service),
        },
        trigger_chain.as_deref().unwrap_or("none"),
        submit_chain.as_deref().unwrap_or("none")
    );

    DeployService::run(
        &clients.cli_ctx,
        DeployServiceArgs {
            component: ComponentSource::Digest(digest),
            trigger,
            trigger_chain,
            trigger_address,
            submit_address,
            trigger_event_name,
            submit,
            submit_chain,
            service_config: None,
        },
    )
    .await
    .unwrap()
    .unwrap()
    .service
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

    let address = SimpleEthTriggerClient::deploy(eth_client.provider.clone())
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

    let simple_submit = SimpleSubmit::deploy(eth_client.provider.clone(), service_manager_address)
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
