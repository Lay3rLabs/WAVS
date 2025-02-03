use std::path::PathBuf;

use anyhow::{Context, Result};
use layer_climb::prelude::Address;
use serde::{Deserialize, Serialize};
use utils::{
    avs_client::SignedData,
    context::AppContext,
    types::{ChainName, Service, Submit},
};
use wavs_cli::{
    command::add_task::{AddTask, AddTaskArgs},
    util::ComponentInput,
};

use super::{
    clients::Clients,
    config::Configs,
    matrix::{AnyService, CosmosService, CrossChainService, EthService},
    services::Services,
};

pub fn run_tests(ctx: AppContext, configs: Configs, clients: Clients, services: Services) {
    // nonce errors, gotta run sequentially :(
    ctx.rt.block_on(async move {
        let mut all: Vec<(AnyService, Service)> = services.lookup.into_iter().collect();
        all.sort_by(|(a, _), (b, _)| match (a, b) {
            // Ethereum should come first, then cross-chain, then cosmos
            // to ensure that we move ethereum blocks forward
            (AnyService::Eth(_), _) => std::cmp::Ordering::Less,
            (_, AnyService::Eth(_)) => std::cmp::Ordering::Greater,
            (AnyService::CrossChain(_), _) => std::cmp::Ordering::Less,
            (_, AnyService::CrossChain(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        });

        for (name, service) in all {
            test_service(name, service, &configs, &clients)
                .await
                .unwrap();
            tracing::info!("Service {:?} passed", name);
        }
    });
}

async fn test_service(
    name: AnyService,
    service: Service,
    configs: &Configs,
    clients: &Clients,
) -> Result<()> {
    let service_id = service.id.to_string();

    tracing::info!("Testing service: {:?}", name);

    let n_workflows = service.workflows.len();

    for workflow_index in 0..n_workflows {
        let (workflow_id, workflow) = service.workflows.iter().nth(workflow_index).unwrap();

        let n_tasks = match &workflow.submit {
            Submit::EthereumContract { chain_name, .. } => {
                let chain = configs
                    .chains
                    .eth
                    .get(chain_name)
                    .context("couldn't get submission chain to detect aggregation")?;
                match chain.aggregator_endpoint.is_some() {
                    true => configs.aggregator.as_ref().unwrap().tasks_quorum,
                    false => 1,
                }
            }
            Submit::None => 1,
        };

        for task_number in 1..=n_tasks {
            let is_final = task_number == n_tasks;

            let signed_data = AddTask::run(
                &clients.cli_ctx,
                AddTaskArgs {
                    service_id: service_id.clone(),
                    workflow_id: Some(workflow_id.to_string()),
                    input: get_input_for_service(name, &service, configs, workflow_index),
                    result_timeout: if is_final {
                        Some(std::time::Duration::from_secs(10))
                    } else {
                        None
                    },
                },
            )
            .await?
            .context("failed to add task")?
            .signed_data;

            if is_final {
                let signed_data = signed_data.context("no signed data returned")?;
                verify_signed_data(name, signed_data, &service, configs, workflow_index)?;
            }
        }
    }

    Ok(())
}

fn get_input_for_service(
    name: AnyService,
    _service: &Service,
    configs: &Configs,
    workflow_index: usize,
) -> ComponentInput {
    let permissions_req = || {
        PermissionsRequest {
            url: "https://httpbin.org/get".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
        .to_vec()
    };
    let input_data = match name {
        AnyService::Eth(name) => match name {
            EthService::ChainTriggerLookup => b"satoshi".to_vec(),
            EthService::CosmosQuery => CosmosQueryRequest::BlockHeight {
                chain_name: configs.chains.cosmos.keys().next().unwrap().clone(),
            }
            .to_vec(),
            EthService::EchoData => b"The times".to_vec(),
            EthService::EchoDataAggregator => b"Chancellor".to_vec(),
            EthService::EchoDataSecondaryChain => b"collapse".to_vec(),
            EthService::Permissions => permissions_req(),
            EthService::Square => SquareRequest { x: 3 }.to_vec(),
            EthService::MultiWorkflow => match workflow_index {
                0 => SquareRequest { x: 3 }.to_vec(),
                1 => b"the first one was nine".to_vec(),
                _ => unimplemented!(),
            },
        },
        AnyService::Cosmos(name) => match name {
            CosmosService::ChainTriggerLookup => b"nakamoto".to_vec(),
            CosmosService::CosmosQuery => CosmosQueryRequest::BlockHeight {
                chain_name: configs.chains.cosmos.keys().next().unwrap().clone(),
            }
            .to_vec(),
            CosmosService::EchoData => b"on brink".to_vec(),
            CosmosService::Permissions => permissions_req(),
            CosmosService::Square => SquareRequest { x: 3 }.to_vec(),
        },
        AnyService::CrossChain(name) => match name {
            CrossChainService::CosmosToEthEchoData => b"hello eth world from cosmos".to_vec(),
        },
    };

    ComponentInput::Raw(input_data)
}

fn verify_signed_data(
    name: AnyService,
    signed_data: SignedData,
    service: &Service,
    configs: &Configs,
    workflow_index: usize,
) -> Result<()> {
    let data = signed_data.data;

    let input_req = || {
        get_input_for_service(name, service, configs, workflow_index)
            .decode()
            .unwrap()
    };

    let expected_data = match name {
        AnyService::Eth(eth_name) => match eth_name {
            // Just echo
            EthService::EchoData
            | EthService::EchoDataSecondaryChain
            | EthService::EchoDataAggregator
            | EthService::ChainTriggerLookup => Some(input_req()),

            EthService::Square => Some(SquareResponse { y: 9 }.to_vec()),

            EthService::MultiWorkflow => match workflow_index {
                0 => Some(SquareResponse { y: 9 }.to_vec()),
                1 => Some(input_req()),
                _ => unimplemented!(),
            },

            EthService::CosmosQuery => {
                let resp: CosmosQueryResponse = serde_json::from_slice(&data).unwrap();
                tracing::info!("Response: {:?}", resp);
                None
            }

            EthService::Permissions => {
                let resp: PermissionsResponse = serde_json::from_slice(&data).unwrap();
                tracing::info!("Response: {:?}", resp);
                None
            }
        },
        AnyService::Cosmos(cosmos_name) => match cosmos_name {
            CosmosService::EchoData | CosmosService::ChainTriggerLookup => Some(
                get_input_for_service(name, service, configs, workflow_index)
                    .decode()
                    .unwrap(),
            ),

            CosmosService::Square => Some(SquareResponse { y: 9 }.to_vec()),

            CosmosService::Permissions => {
                let resp: PermissionsResponse = serde_json::from_slice(&data).unwrap();
                tracing::info!("Response: {:?}", resp);
                None
            }

            CosmosService::CosmosQuery => {
                let resp: CosmosQueryResponse = serde_json::from_slice(&data).unwrap();
                tracing::info!("Response: {:?}", resp);
                None
            }
        },
        AnyService::CrossChain(crosschain_name) => match crosschain_name {
            CrossChainService::CosmosToEthEchoData => Some(
                get_input_for_service(name, service, configs, workflow_index)
                    .decode()
                    .unwrap(),
            ),
        },
    };

    // in some cases we just verify that we could deserialize the data
    // in others, we know what we expect exactly, make sure we got it
    if let Some(expected_data) = expected_data {
        assert_eq!(data, expected_data);

        if let Ok(msg) = String::from_utf8(data) {
            tracing::info!("Response: {}", msg);
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SquareRequest {
    pub x: u64,
}

impl SquareRequest {
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(dead_code)]
pub struct SquareResponse {
    pub y: u64,
}

impl SquareResponse {
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryRequest {
    BlockHeight {
        chain_name: ChainName,
    },
    Balance {
        chain_name: String,
        address: Address,
    },
}

impl CosmosQueryRequest {
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum CosmosQueryResponse {
    BlockHeight(u64),
    Balance(String),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PermissionsRequest {
    pub url: String,
    pub timestamp: u64,
}

impl PermissionsRequest {
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PermissionsResponse {
    pub filename: PathBuf,
    pub contents: String,
    pub filecount: usize,
}
