use std::collections::BTreeMap;

use super::{
    clients::Clients,
    config::Configs,
    digests::Digests,
    matrix::{AnyService, CrossChainService, EthService},
};
use alloy::sol_types::SolEvent;
use utils::{
    context::AppContext,
    eigen_client::CoreAVSAddresses,
    types::{ChainName, Component, ComponentSource, Permissions, Service, Workflow}, ComponentID, WorkflowID,
};
use wavs_cli::{
    args::{CliSubmitKind, CliTriggerKind},
    command::{
        deploy_eigen_core::{DeployEigenCore, DeployEigenCoreArgs},
        deploy_service::{DeployService, DeployServiceArgs},
    },
};

#[derive(Default)]
pub struct Services {
    #[allow(dead_code)]
    pub eth_eigen_core: BTreeMap<ChainName, CoreAVSAddresses>,
    pub lookup: BTreeMap<AnyService, Service>,
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
            // hrmf, "nonce too low" errors, gotta go sequentially...

            for chain in chain_names
                .eth
                .iter()
                .chain(chain_names.eth_aggregator.iter())
            {
                let chain = chain.clone();
                tracing::info!("Deploying Eigen Core contracts on {chain}");
                let DeployEigenCore { addresses, .. } = DeployEigenCore::run(
                    &clients.cli_ctx,
                    DeployEigenCoreArgs {
                        register_operator: true,
                        chain: chain.clone(),
                    },
                )
                .await
                .unwrap();

                eth_eigen_core.insert(chain, addresses);
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

            // nonce errors here too, gotta go sequentially :/
            for service_kind in all_services {

                let service = match service_kind {
                    AnyService::Eth(EthService::MultiWorkflow) => {
                        deploy_service_raw(service_kind, clients, digests, &chain_names).await
                    },
                    _ => deploy_service_simple(service_kind, clients, digests, &chain_names).await
                };

                lookup.insert(service_kind, service);
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
    clients: &Clients,
    digests: &Digests,
    chain_names: &ChainNames,
) -> Service {
    let digest = digests.lookup.get(&service.into()).unwrap().clone();

    let trigger = match service {
        AnyService::Eth(_) => CliTriggerKind::EthContractEvent,
        AnyService::Cosmos(_) => CliTriggerKind::CosmosContractEvent,
        AnyService::CrossChain(service) => match service {
            CrossChainService::CosmosToEthEchoData => CliTriggerKind::CosmosContractEvent,
        },
    };

    let trigger_event_name = match trigger {
        CliTriggerKind::EthContractEvent => Some(const_hex::encode(
            wavs_cli::clients::example_eth_client::example_trigger::NewTrigger::SIGNATURE_HASH,
        )),
        CliTriggerKind::CosmosContractEvent => {
            Some(wavs_cli::clients::example_cosmos_client::NewMessageEvent::KEY.to_string())
        }
    };

    let submit = match service {
        _ => CliSubmitKind::SimpleEthContract,
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

    let submit_chain = match submit {
        CliSubmitKind::SimpleEthContract => match trigger {
            CliTriggerKind::EthContractEvent => trigger_chain.clone(), // not strictly necessary, just easier to reason about same-chain
            CliTriggerKind::CosmosContractEvent => Some(chain_names.eth[0].clone()), // always eth for now
        },
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
            register_operator: true,
            component: ComponentSource::Digest(digest),
            trigger,
            trigger_chain,
            trigger_address: None,
            submit_address: None,
            trigger_event_name,
            cosmos_trigger_code_id: None,
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

    let mut service = Service::new_simple(
        "",
        trigger,
        digest,
        submit,
        Some(config),
    );


    let component_id1 = ComponentID::new("component1").unwrap();
    let component_id2 = ComponentID::new("component2").unwrap();
    let workflow_id1 = WorkflowID::new("workflow1").unwrap();
    let workflow_id2 = WorkflowID::new("workflow2").unwrap();

    let workflow = Workflow {
        trigger,
        component: component_id,
        submit,
    };

    let component = Component {
        wasm: component_digest,
        permissions: Permissions::default(),
    };

    let components = BTreeMap::from([(workflow.component.clone(), component)]);

    let workflows = BTreeMap::from([(workflow_id, workflow)]);

    for component in service.components.values_mut() {
        component.permissions = Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        }
    }

    let service = Service {
        id: ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string()).unwrap(),
        name: "".to_string(),

    }

    todo!()
}

#[derive(Default)]
struct ChainNames {
    eth: Vec<ChainName>,
    eth_aggregator: Vec<ChainName>,
    cosmos: Vec<ChainName>,
}
