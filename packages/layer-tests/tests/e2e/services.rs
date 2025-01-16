use std::collections::HashMap;

use super::{clients::Clients, config::Configs, digests::Digests, eth, matrix::TestMatrix};
use futures::{stream::FuturesUnordered, StreamExt};
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
    pub cross_chain: CrossChainServices,
}

#[derive(Default)]
pub struct EthServices {
    pub chain_trigger_lookup: Option<(ServiceName, DeployService)>,
    pub cosmos_query: Option<(ServiceName, DeployService)>,
    pub echo_data: Option<(ServiceName, DeployService)>,
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
    pub fn new(
        ctx: AppContext,
        configs: &Configs,
        clients: &Clients,
        digests: &Digests,
        matrix: &TestMatrix,
    ) -> Self {
        ctx.rt.block_on(async move {
            let eth_chain_names = configs.chains.eth.keys().cloned().collect::<Vec<_>>();
            let cosmos_chain_names = configs.chains.cosmos.keys().cloned().collect::<Vec<_>>();

            let mut services = Self::default();

            // hrmf, "nonce too low" errors, gotta go sequentially...

            for chain in eth_chain_names.iter() {
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

                services.eth_eigen_core.insert(chain.to_string(), addresses);
            }

            // nonce errors :(
            // let mut futures = FuturesUnordered::new();

            let mut futures = Vec::new();
            if matrix.eth.chain_trigger_lookup {
                futures.push(
                    deploy_service(
                        ServiceName::EthChainTriggerLookup,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.eth.cosmos_query {
                futures.push(
                    deploy_service(
                        ServiceName::EthCosmosQuery,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.eth.echo_data {
                futures.push(
                    deploy_service(
                        ServiceName::EthEchoData,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.eth.echo_data_aggregator {
                futures.push(
                    deploy_service(
                        ServiceName::EthEchoDataAggregator,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.eth.permissions {
                futures.push(
                    deploy_service(
                        ServiceName::EthPermissions,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.eth.square {
                futures.push(
                    deploy_service(
                        ServiceName::EthSquare,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.cosmos.chain_trigger_lookup {
                futures.push(
                    deploy_service(
                        ServiceName::CosmosChainTriggerLookup,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.cosmos.cosmos_query {
                futures.push(
                    deploy_service(
                        ServiceName::CosmosCosmosQuery,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.cosmos.echo_data {
                futures.push(
                    deploy_service(
                        ServiceName::CosmosEchoData,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.cosmos.permissions {
                futures.push(
                    deploy_service(
                        ServiceName::CosmosPermissions,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            if matrix.cosmos.square {
                futures.push(
                    deploy_service(
                        ServiceName::CosmosSquare,
                        &clients,
                        &digests,
                        &eth_chain_names,
                        &cosmos_chain_names,
                    )
                    .await,
                );
            }

            for service in futures {
                match service.0 {
                    ServiceName::EthChainTriggerLookup => {
                        services.eth.chain_trigger_lookup = Some(service)
                    }
                    ServiceName::EthCosmosQuery => services.eth.cosmos_query = Some(service),
                    ServiceName::EthEchoData => services.eth.echo_data = Some(service),
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
    eth_chain_names: &[String],
    cosmos_chain_names: &[String],
) -> (ServiceName, DeployService) {
    let digest = match name {
        ServiceName::EthChainTriggerLookup | ServiceName::CosmosChainTriggerLookup => {
            digests.chain_trigger_lookup.clone().unwrap()
        }

        ServiceName::EthCosmosQuery | ServiceName::CosmosCosmosQuery => {
            digests.cosmos_query.clone().unwrap()
        }

        ServiceName::EthEchoData | ServiceName::CosmosEchoData => {
            digests.echo_data.clone().unwrap()
        }

        ServiceName::EthEchoDataAggregator => digests.echo_data.clone().unwrap(),

        ServiceName::EthPermissions | ServiceName::CosmosPermissions => {
            digests.permissions.clone().unwrap()
        }

        ServiceName::EthSquare | ServiceName::CosmosSquare => digests.square.clone().unwrap(),
    };

    let trigger = match name {
        ServiceName::EthChainTriggerLookup
        | ServiceName::EthCosmosQuery
        | ServiceName::EthEchoData
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
        CliTriggerKind::SimpleEthContract => Some(eth_chain_names[0].clone()),
        CliTriggerKind::SimpleCosmosContract => Some(cosmos_chain_names[0].clone()),
    };

    let submit_chain = match submit {
        CliSubmitKind::SimpleEthContract => Some(eth_chain_names[0].clone()),
    };

    let aggregate = match name {
        ServiceName::EthEchoDataAggregator => true,
        _ => false,
    };

    tracing::info!("Deploying Service {:?}", name);

    let service = DeployService::run(
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
            aggregate,
        },
    )
    .await
    .unwrap()
    .unwrap();

    (name, service)
}
