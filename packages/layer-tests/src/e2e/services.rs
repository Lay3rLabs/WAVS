use std::collections::HashMap;

use super::{clients::Clients, config::Configs, digests::Digests};
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
    pub eth_eigen_core: HashMap<String, CoreAVSAddresses>,
    pub eth: EthServices,
    pub cosmos: CosmosServices,
    pub _cross_chain: CrossChainServices,
}

#[derive(Default)]
pub struct EthServices {
    pub chain_trigger_lookup: Option<(ServiceName, DeployService)>,
    pub cosmos_query: Option<(ServiceName, DeployService)>,
    pub echo_data: Option<(ServiceName, DeployService)>,
    pub echo_data_multichain_1: Option<(ServiceName, DeployService)>,
    pub echo_data_multichain_2: Option<(ServiceName, DeployService)>,
    pub echo_data_aggregator: Option<(ServiceName, DeployService)>,
    pub permissions: Option<(ServiceName, DeployService)>,
    pub square: Option<(ServiceName, DeployService)>,
}

#[derive(Default)]
pub struct CosmosServices {
    pub chain_trigger_lookup: Option<(ServiceName, DeployService)>,
    pub cosmos_query: Option<(ServiceName, DeployService)>,
    pub echo_data: Option<(ServiceName, DeployService)>,
    pub permissions: Option<(ServiceName, DeployService)>,
    pub square: Option<(ServiceName, DeployService)>,
}

#[derive(Default)]
pub struct CrossChainServices {}

impl Services {
    pub fn new(ctx: AppContext, configs: &Configs, clients: &Clients, digests: &Digests) -> Self {
        ctx.rt.block_on(async move {
            let matrix = &configs.test_config.matrix;

            let mut chain_names = ChainNames::default();

            for (chain_name, chain) in configs.chains.eth.iter() {
                if chain.aggregator_endpoint.is_some() {
                    chain_names.eth_aggregator.push(chain_name.clone());
                } else {
                    chain_names.eth.push(chain_name.clone());
                }
            }

            chain_names.cosmos = configs.chains.cosmos.keys().cloned().collect::<Vec<_>>();

            let mut services = Self::default();

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

                services.eth_eigen_core.insert(chain, addresses);
            }

            let mut names: Vec<ServiceName> = Vec::new();
            if matrix.eth.chain_trigger_lookup {
                names.push(ServiceName::EthChainTriggerLookup);
            }

            if matrix.eth.cosmos_query {
                names.push(ServiceName::EthCosmosQuery);
            }

            if matrix.eth.echo_data {
                names.push(ServiceName::EthEchoData);
            }

            if matrix.eth.echo_data_multichain {
                names.push(ServiceName::EthEchoDataMultichain1);
                names.push(ServiceName::EthEchoDataMultichain2);
            }

            if matrix.eth.echo_data_aggregator {
                names.push(ServiceName::EthEchoDataAggregator);
            }

            if matrix.eth.permissions {
                names.push(ServiceName::EthPermissions);
            }

            if matrix.eth.square {
                names.push(ServiceName::EthSquare);
            }

            if matrix.cosmos.chain_trigger_lookup {
                names.push(ServiceName::CosmosChainTriggerLookup);
            }

            if matrix.cosmos.cosmos_query {
                names.push(ServiceName::CosmosCosmosQuery);
            }

            if matrix.cosmos.echo_data {
                names.push(ServiceName::CosmosEchoData);
            }

            if matrix.cosmos.permissions {
                names.push(ServiceName::CosmosPermissions);
            }

            if matrix.cosmos.square {
                names.push(ServiceName::CosmosSquare);
            }

            // nonce errors :(
            // let mut futures = FuturesUnordered::new();

            for name in names {
                let res = deploy_service(name, clients, digests, &chain_names).await;

                let service = (name, res);

                match name {
                    ServiceName::EthChainTriggerLookup => {
                        services.eth.chain_trigger_lookup = Some(service)
                    }
                    ServiceName::EthCosmosQuery => services.eth.cosmos_query = Some(service),
                    ServiceName::EthEchoData => services.eth.echo_data = Some(service),
                    ServiceName::EthEchoDataMultichain1 => {
                        services.eth.echo_data_multichain_1 = Some(service)
                    }
                    ServiceName::EthEchoDataMultichain2 => {
                        services.eth.echo_data_multichain_2 = Some(service)
                    }
                    ServiceName::EthEchoDataAggregator => {
                        services.eth.echo_data_aggregator = Some(service)
                    }
                    ServiceName::EthPermissions => services.eth.permissions = Some(service),
                    ServiceName::EthSquare => services.eth.square = Some(service),
                    ServiceName::CosmosChainTriggerLookup => {
                        services.cosmos.chain_trigger_lookup = Some(service)
                    }
                    ServiceName::CosmosCosmosQuery => services.cosmos.cosmos_query = Some(service),
                    ServiceName::CosmosEchoData => services.cosmos.echo_data = Some(service),
                    ServiceName::CosmosPermissions => services.cosmos.permissions = Some(service),
                    ServiceName::CosmosSquare => services.cosmos.square = Some(service),
                }
            }

            services
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ServiceName {
    EthChainTriggerLookup,
    EthCosmosQuery,
    EthEchoData,
    EthEchoDataMultichain1,
    EthEchoDataMultichain2,
    EthEchoDataAggregator,
    EthPermissions,
    EthSquare,
    CosmosChainTriggerLookup,
    CosmosCosmosQuery,
    CosmosEchoData,
    CosmosPermissions,
    CosmosSquare,
}

async fn deploy_service(
    name: ServiceName,
    clients: &Clients,
    digests: &Digests,
    chain_names: &ChainNames,
) -> DeployService {
    let digest = match name {
        ServiceName::EthChainTriggerLookup | ServiceName::CosmosChainTriggerLookup => {
            digests.chain_trigger_lookup.clone().unwrap()
        }

        ServiceName::EthCosmosQuery | ServiceName::CosmosCosmosQuery => {
            digests.cosmos_query.clone().unwrap()
        }

        ServiceName::EthEchoData
        | ServiceName::CosmosEchoData
        | ServiceName::EthEchoDataMultichain1
        | ServiceName::EthEchoDataMultichain2
        | ServiceName::EthEchoDataAggregator => digests.echo_data.clone().unwrap(),

        ServiceName::EthPermissions | ServiceName::CosmosPermissions => {
            digests.permissions.clone().unwrap()
        }

        ServiceName::EthSquare | ServiceName::CosmosSquare => digests.square.clone().unwrap(),
    };

    let trigger = match name {
        ServiceName::EthChainTriggerLookup
        | ServiceName::EthCosmosQuery
        | ServiceName::EthEchoData
        | ServiceName::EthEchoDataMultichain1
        | ServiceName::EthEchoDataMultichain2
        | ServiceName::EthEchoDataAggregator
        | ServiceName::EthPermissions
        | ServiceName::EthSquare => CliTriggerKind::SimpleEthContract,

        ServiceName::CosmosChainTriggerLookup
        | ServiceName::CosmosCosmosQuery
        | ServiceName::CosmosEchoData
        | ServiceName::CosmosPermissions
        | ServiceName::CosmosSquare => CliTriggerKind::SimpleCosmosContract,
    };

    let submit = match name {
        _ => CliSubmitKind::SimpleEthContract,
    };

    let trigger_chain = match trigger {
        CliTriggerKind::SimpleEthContract => match name {
            ServiceName::EthEchoDataAggregator => Some(chain_names.eth_aggregator[0].clone()),
            ServiceName::EthEchoDataMultichain2 => Some(chain_names.eth[1].clone()),
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
        "Deploying Service {:?} on trigger_chain: {} submit_chain: {}",
        name,
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
