use std::path::PathBuf;

use anyhow::{Context, Result};
use layer_climb::prelude::Address;
use serde::{Deserialize, Serialize};
use utils::avs_client::SignedData;
use wavs::AppContext;
use wavs_cli::{
    command::{
        add_task::{AddTask, AddTaskArgs},
        deploy_service::DeployService,
    },
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
        for (name, service) in services.lookup.into_iter() {
            test_service(name, service, &configs, &clients)
                .await
                .unwrap();
            tracing::info!("Service {:?} passed", name);
        }
    });
}

async fn test_service(
    name: AnyService,
    service: DeployService,
    configs: &Configs,
    clients: &Clients,
) -> Result<()> {
    let service_id = service.service_id.to_string();
    let (workflow_id, workflow) = service.workflows.into_iter().next().unwrap().clone();

    tracing::info!("Testing service: {:?}", name);

    let n_tasks = match workflow.submit {
        wavs_cli::deploy::ServiceSubmitInfo::EigenLayer { chain_name, .. } => {
            let chain = configs
                .chains
                .eth
                .get(&chain_name)
                .context("couldn't get submission chain to detect aggregation")?;
            match chain.aggregator_endpoint.is_some() {
                true => configs.aggregator.as_ref().unwrap().tasks_quorum,
                false => 1,
            }
        }
    };

    for task_number in 1..=n_tasks {
        let is_final = task_number == n_tasks;

        let signed_data = AddTask::run(
            &clients.cli_ctx,
            AddTaskArgs {
                service_id: service_id.clone(),
                workflow_id: Some(workflow_id.to_string()),
                input: get_input_for_service(name),
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
            verify_signed_data(name, signed_data)?;
        }
    }

    Ok(())
}

fn get_input_for_service(name: AnyService) -> ComponentInput {
    let input_data = match name {
        AnyService::Eth(name) => match name {
            EthService::ChainTriggerLookup => todo!(),
            EthService::CosmosQuery => CosmosQueryRequest::BlockHeight.to_vec(),
            EthService::EchoData => b"The times".to_vec(),
            EthService::EchoDataAggregator => b"Chancellor".to_vec(),
            EthService::EchoDataSecondaryChain => b"collapse".to_vec(),
            EthService::Permissions => PermissionsRequest {
                url: "https://httpbin.org/get".to_string(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            }
            .to_vec(),
            EthService::Square => SquareRequest { x: 3 }.to_vec(),
        },
        AnyService::Cosmos(name) => match name {
            CosmosService::ChainTriggerLookup => todo!(),
            CosmosService::CosmosQuery => CosmosQueryRequest::BlockHeight.to_vec(),
            CosmosService::EchoData => b"on brink".to_vec(),
            CosmosService::Permissions => todo!(),
            CosmosService::Square => SquareRequest { x: 3 }.to_vec(),
        },
        AnyService::CrossChain(name) => match name {
            CrossChainService::CosmosToEthEchoData => b"hello eth world from cosmos".to_vec(),
        },
    };

    ComponentInput::Raw(input_data)
}

fn verify_signed_data(name: AnyService, signed_data: SignedData) -> Result<()> {
    let data = signed_data.data;

    let expected_data = match name {
        AnyService::Eth(eth_name) => match eth_name {
            // Just echo
            EthService::EchoData
            | EthService::EchoDataSecondaryChain
            | EthService::EchoDataAggregator => Some(get_input_for_service(name).decode().unwrap()),

            EthService::Square => Some(SquareResponse { y: 9 }.to_vec()),

            EthService::CosmosQuery => {
                let _: CosmosQueryResponse = serde_json::from_slice(&data).unwrap();
                None
            }

            EthService::Permissions => {
                let _: PermissionsResponse = serde_json::from_slice(&data).unwrap();
                None
            }

            EthService::ChainTriggerLookup => todo!(),
        },
        AnyService::Cosmos(cosmos_name) => match cosmos_name {
            CosmosService::EchoData => Some(get_input_for_service(name).decode().unwrap()),

            CosmosService::Square => Some(SquareResponse { y: 9 }.to_vec()),

            CosmosService::Permissions => {
                let _: PermissionsResponse = serde_json::from_slice(&data).unwrap();
                None
            }

            CosmosService::CosmosQuery => {
                let _: CosmosQueryResponse = serde_json::from_slice(&data).unwrap();
                None
            }

            CosmosService::ChainTriggerLookup => todo!(),
        },
        AnyService::CrossChain(crosschain_name) => match crosschain_name {
            CrossChainService::CosmosToEthEchoData => {
                Some(get_input_for_service(name).decode().unwrap())
            }
        },
    };

    // in some cases we just verify that we could deserialize the data
    // in others, we know what we expect exactly, make sure we got it
    if let Some(expected_data) = expected_data {
        assert_eq!(data, expected_data);
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
    BlockHeight,
    Balance { address: Address },
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
