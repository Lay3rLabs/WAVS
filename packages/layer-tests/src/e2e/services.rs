use std::collections::BTreeMap;

use crate::{
    e2e::digests::DigestName, example_cosmos_client::SimpleCosmosTriggerClient,
    example_eth_client::example_trigger::SimpleTrigger,
};

use super::{
    clients::Clients,
    config::Configs,
    digests::Digests,
    matrix::{AnyService, CrossChainService, EthService},
};
use crate::example_eth_client::{example_submit::SimpleSubmit, SimpleEthTriggerClient};
use alloy::sol_types::SolEvent;
use utils::{
    avs_client::layer_service_aggregator::WavsServiceAggregator, context::AppContext,
    eigen_client::CoreAVSAddresses, filesystem::workspace_path,
};
use wavs_cli::{
    args::{CliSubmitKind, CliTriggerKind},
    command::{
        deploy_eigen_core::{DeployEigenCore, DeployEigenCoreArgs},
        deploy_eigen_service_manager::{DeployEigenServiceManager, DeployEigenServiceManagerArgs},
        deploy_service::{DeployService, DeployServiceArgs},
        deploy_service_raw::{DeployServiceRaw, DeployServiceRawArgs},
    },
};
use wavs_types::{
    AllowedHostPermission, ByteArray, ChainName, Component, ComponentID, ComponentSource,
    Permissions, Service, ServiceConfig, ServiceID, ServiceStatus, Submit, Trigger, Workflow,
    WorkflowID,
};

#[derive(Default)]
pub struct Services {
    #[allow(dead_code)]
    pub eth_eigen_core: BTreeMap<ChainName, CoreAVSAddresses>,
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

            let mut eth_eigen_core = BTreeMap::default();
            let mut eth_service_managers = BTreeMap::default();
            // hrmf, "nonce too low" errors, gotta go sequentially...

            for chain in chain_names
                .eth
                .iter()
                .chain(chain_names.eth_aggregator.iter())
            {
                let chain = chain.clone();
                tracing::info!("Deploying Eigen Core contracts on {chain}");
                let DeployEigenCore {
                    addresses: core_addresses,
                    ..
                } = DeployEigenCore::run(
                    &clients.cli_ctx,
                    DeployEigenCoreArgs {
                        register_operator: true,
                        chain: chain.clone(),
                    },
                )
                .await
                .unwrap();

                eth_eigen_core.insert(chain.clone(), core_addresses);

                tracing::info!("Deploying Eigen Service manager on {chain}");
                let DeployEigenServiceManager {
                    address: service_manager_address,
                    ..
                } = DeployEigenServiceManager::run(
                    &clients.cli_ctx,
                    DeployEigenServiceManagerArgs {
                        chain: chain.clone(),
                        register_operator: true,
                    },
                )
                .await
                .unwrap();

                eth_service_managers.insert(chain.clone(), service_manager_address);
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
                        deploy_service_raw(service_kind, clients, digests, &chain_names).await
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

            Self {
                eth_eigen_core,
                lookup,
            }
        })
    }
}

async fn deploy_service_simple(
    service: AnyService,
    configs: &Configs,
    clients: &Clients,
    digests: &Digests,
    chain_names: &ChainNames,
    cosmos_code_ids: &mut BTreeMap<ChainName, u64>,
    eth_service_managers: &BTreeMap<ChainName, alloy::primitives::Address>,
) -> Service {
    let digest_name = Vec::<DigestName>::from(service)[0];
    let digest = digests.lookup.get(&digest_name).unwrap().clone();

    let trigger = match service {
        AnyService::Eth(_) => CliTriggerKind::EthContractEvent,
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
    };

    let trigger_address = match trigger {
        CliTriggerKind::EthContractEvent => {
            let client = clients
                .cli_ctx
                .get_eth_client(trigger_chain.as_ref().unwrap())
                .unwrap();

            tracing::info!("Deploying new eth trigger contract");
            Some(
                SimpleTrigger::deploy(client.eth.provider)
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
    };

    let submit_chain = match submit {
        CliSubmitKind::EthServiceHandler => match trigger {
            CliTriggerKind::EthContractEvent => trigger_chain.clone(), // not strictly necessary, just easier to reason about same-chain
            CliTriggerKind::CosmosContractEvent => Some(chain_names.eth[0].clone()), // always eth for now
        },
        CliSubmitKind::None => None,
    };

    let submit_address = match submit {
        CliSubmitKind::EthServiceHandler => {
            let submit_chain = submit_chain.as_ref().unwrap();
            let client = clients.cli_ctx.get_eth_client(submit_chain).unwrap();
            let service_manager_address = *eth_service_managers.get(submit_chain).unwrap();

            tracing::info!("Deploying new eth submit contract");
            let handler_address =
                *SimpleSubmit::deploy(client.eth.provider.clone(), service_manager_address)
                    .await
                    .unwrap()
                    .address();

            let chain = configs.chains.eth.get(submit_chain).unwrap();

            match chain.aggregator_endpoint.is_some() {
                true => Some(
                    WavsServiceAggregator::deploy(client.eth.provider.clone(), handler_address)
                        .await
                        .unwrap()
                        .address()
                        .to_string(),
                ),
                false => Some(handler_address.to_string()),
            }
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
        wasm: digests.lookup.get(&digest_names[0]).unwrap().clone(),
        permissions: Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        },
    };

    let component2 = Component {
        wasm: digests.lookup.get(&digest_names[1]).unwrap().clone(),
        permissions: Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        },
    };

    let submit1 = deploy_submit_raw(clients, chain_names).await;
    let submit2 = deploy_submit_raw(clients, chain_names).await;

    let workflow_id1 = WorkflowID::new("workflow1").unwrap();
    let workflow_id2 = WorkflowID::new("workflow2").unwrap();

    let workflow1 = Workflow {
        trigger: trigger1,
        component: component_id1,
        submit: submit1,
        fuel_limit: None,
    };

    let workflow2 = Workflow {
        trigger: trigger2,
        component: component_id2,
        submit: submit2,
        fuel_limit: None,
    };

    let components = BTreeMap::from([
        (workflow1.component.clone(), component1),
        (workflow2.component.clone(), component2),
    ]);

    let workflows = BTreeMap::from([(workflow_id1, workflow1), (workflow_id2, workflow2)]);

    let service = Service {
        id: ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap(),
        name: "".to_string(),
        components,
        workflows,
        status: ServiceStatus::Active,
        config: ServiceConfig::default(),
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
    let eigen_client = clients.cli_ctx.get_eth_client(&chain_name).unwrap().clone();
    let event_hash = *crate::example_eth_client::example_trigger::NewTrigger::SIGNATURE_HASH;

    let address = SimpleEthTriggerClient::deploy(eigen_client.eth.provider.clone())
        .await
        .unwrap();

    Trigger::EthContractEvent {
        chain_name,
        address,
        event_hash: ByteArray::new(event_hash),
    }
}

async fn deploy_submit_raw(clients: &Clients, chain_names: &ChainNames) -> Submit {
    let chain_name = chain_names.eth[0].clone();
    let eigen_client = clients.cli_ctx.get_eth_client(&chain_name).unwrap().clone();

    let res = DeployEigenServiceManager::run(
        &clients.cli_ctx,
        DeployEigenServiceManagerArgs {
            chain: chain_name.clone(),
            register_operator: true,
        },
    )
    .await
    .unwrap();

    let DeployEigenServiceManager {
        address: service_manager_address,
        ..
    } = res;

    let simple_submit =
        SimpleSubmit::deploy(eigen_client.eth.provider.clone(), service_manager_address)
            .await
            .unwrap();

    Submit::EthereumContract {
        chain_name,
        address: *simple_submit.address(),
        max_gas: None,
    }
}

#[derive(Default)]
struct ChainNames {
    eth: Vec<ChainName>,
    eth_aggregator: Vec<ChainName>,
    cosmos: Vec<ChainName>,
}
