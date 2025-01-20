use std::collections::BTreeMap;

use super::{
    clients::Clients,
    config::Configs,
    digests::Digests,
    matrix::{AnyService, CrossChainService, EthService},
};
use utils::{context::AppContext, eigen_client::CoreAVSAddresses};
use wavs_cli::{
    args::{CliSubmitKind, CliTriggerKind},
    command::{
        deploy_eigen_core::{DeployEigenCore, DeployEigenCoreArgs},
        deploy_service::{ComponentSource, DeployService, DeployServiceArgs},
    },
};

#[derive(Default)]
pub struct Services {
    #[allow(dead_code)]
    pub eth_eigen_core: BTreeMap<String, CoreAVSAddresses>,
    pub lookup: BTreeMap<AnyService, DeployService>,
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
                let chain = chain.to_string();
                tracing::info!("Deploying Eigen Core contracts on {chain}");
                let DeployEigenCore { addresses } = DeployEigenCore::run(
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
            for service in all_services {
                let res = deploy_service(service, clients, digests, &chain_names).await;
                lookup.insert(service, res);
            }

            Self {
                eth_eigen_core,
                lookup,
            }
        })
    }
}

async fn deploy_service(
    service: AnyService,
    clients: &Clients,
    digests: &Digests,
    chain_names: &ChainNames,
) -> DeployService {
    let digest = digests.lookup.get(&service.into()).unwrap().clone();

    let trigger = match service {
        AnyService::Eth(_) => CliTriggerKind::SimpleEthContract,
        AnyService::Cosmos(_) => CliTriggerKind::SimpleCosmosContract,
        AnyService::CrossChain(service) => match service {
            CrossChainService::CosmosToEthEchoData => CliTriggerKind::SimpleEthContract,
        },
    };

    let submit = match service {
        _ => CliSubmitKind::SimpleEthContract,
    };

    let trigger_chain = match trigger {
        CliTriggerKind::SimpleEthContract => match service {
            AnyService::Eth(EthService::EchoDataAggregator) => {
                Some(chain_names.eth_aggregator[0].clone())
            }
            AnyService::Eth(EthService::EchoDataSecondaryChain) => Some(chain_names.eth[1].clone()),
            _ => Some(chain_names.eth[0].clone()),
        },
        CliTriggerKind::SimpleCosmosContract => Some(chain_names.cosmos[0].clone()),
    };

    let submit_chain = match submit {
        CliSubmitKind::SimpleEthContract => match trigger {
            CliTriggerKind::SimpleEthContract => trigger_chain.clone(), // not strictly necessary, just convenient
            CliTriggerKind::SimpleCosmosContract => Some(chain_names.eth[0].clone()),
        },
    };

    tracing::info!(
        "Deploying Service {} on trigger_chain: {} submit_chain: {}",
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
            cosmos_trigger_code_id: None,
            submit,
            submit_chain,
            service_config: None,
        },
    )
    .await
    .unwrap()
    .unwrap()
}

#[derive(Default)]
struct ChainNames {
    eth: Vec<String>,
    eth_aggregator: Vec<String>,
    cosmos: Vec<String>,
}
