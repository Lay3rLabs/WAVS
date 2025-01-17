use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
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
    services::{ServiceName, Services},
};

pub fn run_tests(ctx: AppContext, configs: Configs, clients: Clients, services: Services) {
    let mut services_to_run = Vec::new();

    if let Some(service) = services.eth.chain_trigger_lookup {
        services_to_run.push(service);
    }
    if let Some(service) = services.eth.cosmos_query {
        services_to_run.push(service);
    }
    if let Some(service) = services.eth.echo_data {
        services_to_run.push(service);
    }
    if let Some(service) = services.eth.echo_data_multichain_1 {
        services_to_run.push(service);
    }
    if let Some(service) = services.eth.echo_data_multichain_2 {
        services_to_run.push(service);
    }
    if let Some(service) = services.eth.echo_data_aggregator {
        services_to_run.push(service);
    }
    if let Some(service) = services.eth.permissions {
        services_to_run.push(service);
    }
    if let Some(service) = services.eth.square {
        services_to_run.push(service);
    }
    if let Some(service) = services.cosmos.chain_trigger_lookup {
        services_to_run.push(service);
    }
    if let Some(service) = services.cosmos.cosmos_query {
        services_to_run.push(service);
    }
    if let Some(service) = services.cosmos.echo_data {
        services_to_run.push(service);
    }
    if let Some(service) = services.cosmos.permissions {
        services_to_run.push(service);
    }
    if let Some(service) = services.cosmos.square {
        services_to_run.push(service);
    }

    // nonce errors :(
    // let mut futures = FuturesUnordered::new();

    ctx.rt.block_on(async move {
        for service in services_to_run {
            let name = test_service(service, &configs, &clients).await.unwrap();
            tracing::info!("Service {:?} passed", name);
        }
    });
}

async fn test_service(
    (name, service): (ServiceName, DeployService),
    configs: &Configs,
    clients: &Clients,
) -> Result<ServiceName> {
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

    Ok(name)
}

fn get_input_for_service(name: ServiceName) -> ComponentInput {
    let input_data = match name {
        ServiceName::EthChainTriggerLookup => todo!(),
        ServiceName::EthCosmosQuery => CosmosQueryRequest::BlockHeight.to_vec(),
        ServiceName::EthEchoData => b"The times".to_vec(),
        ServiceName::EthEchoDataAggregator => b"Chancellor".to_vec(),
        ServiceName::EthEchoDataMultichain1 => b"collapse".to_vec(),
        ServiceName::EthEchoDataMultichain2 => b"satoshi".to_vec(),
        ServiceName::EthPermissions => PermissionsRequest {
            url: "https://httpbin.org/get".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
        .to_vec(),
        ServiceName::EthSquare => SquareRequest { x: 3 }.to_vec(),
        ServiceName::CosmosChainTriggerLookup => todo!(),
        ServiceName::CosmosCosmosQuery => CosmosQueryRequest::BlockHeight.to_vec(),
        ServiceName::CosmosEchoData => b"on brink".to_vec(),
        ServiceName::CosmosPermissions => todo!(),
        ServiceName::CosmosSquare => SquareRequest { x: 3 }.to_vec(),
    };

    ComponentInput::Raw(input_data)
}

fn verify_signed_data(name: ServiceName, signed_data: SignedData) -> Result<()> {
    let expected_data = match name {
        ServiceName::EthChainTriggerLookup => todo!(),
        ServiceName::CosmosChainTriggerLookup => todo!(),
        ServiceName::EthSquare => SquareResponse { y: 9 }.to_vec(),
        ServiceName::CosmosSquare => SquareResponse { y: 9 }.to_vec(),
        // just echo
        ServiceName::EthEchoData
        | ServiceName::EthEchoDataMultichain1
        | ServiceName::EthEchoDataMultichain2
        | ServiceName::EthEchoDataAggregator
        | ServiceName::CosmosEchoData => match get_input_for_service(name) {
            ComponentInput::Raw(data) => data,
            _ => unreachable!(),
        },
        // these are not static, handled specially
        ServiceName::EthCosmosQuery => Vec::new(),
        ServiceName::CosmosCosmosQuery => Vec::new(),
        ServiceName::EthPermissions => Vec::new(),
        ServiceName::CosmosPermissions => todo!(),
    };

    let data = signed_data.data;

    match name {
        ServiceName::EthCosmosQuery | ServiceName::CosmosCosmosQuery => {
            let _: CosmosQueryResponse = serde_json::from_slice(&data).unwrap();
            return Ok(());
        }
        ServiceName::EthPermissions | ServiceName::CosmosPermissions => {
            let _: PermissionsResponse = serde_json::from_slice(&data).unwrap();
            return Ok(());
        }
        _ => {}
    }

    if data != expected_data {
        Err(anyhow!("did not receive expected data in {:?}", name))
    } else {
        Ok(())
    }
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
