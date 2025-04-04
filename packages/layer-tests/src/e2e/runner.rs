use std::path::PathBuf;

use alloy::{
    primitives::{eip191_hash_message, keccak256},
    signers::k256::ecdsa::SigningKey,
    sol_types::SolValue,
};
use anyhow::{Context, Result};
use layer_climb::prelude::Address;
use serde::{Deserialize, Serialize};
use utils::context::AppContext;
use wavs_types::{ChainName, EthereumContractSubmission, Service, SigningKeyResponse, Submit};

use crate::e2e::add_task::{add_task, wait_for_task_to_land};

use super::{
    add_task::SignedData,
    clients::Clients,
    config::Configs,
    matrix::{AnyService, CosmosService, CrossChainService, EthService},
    services::Services,
};

pub fn run_tests(ctx: AppContext, configs: Configs, clients: Clients, services: Services) {
    // nonce errors, gotta run sequentially :(
    ctx.rt.block_on(async move {
        let mut all: Vec<(AnyService, Vec<Service>)> = services.lookup.into_iter().collect();
        all.sort_by(|(a, _), (b, _)| match (a, b) {
            // Ethereum should come first, then cross-chain, then cosmos
            // to ensure that we move ethereum blocks forward
            (AnyService::Eth(_), _) => std::cmp::Ordering::Less,
            (_, AnyService::Eth(_)) => std::cmp::Ordering::Greater,
            (AnyService::CrossChain(_), _) => std::cmp::Ordering::Less,
            (_, AnyService::CrossChain(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        });

        for (name, services) in all {
            test_service(name, services, &configs, &clients)
                .await
                .unwrap();
            tracing::info!("Service {:?} passed", name);
        }
    });
}

async fn test_service(
    name: AnyService,
    services: Vec<Service>,
    configs: &Configs,
    clients: &Clients,
) -> Result<()> {
    let service = services.first().unwrap();
    let service_id = service.id.to_string();

    tracing::info!("Testing service: {:?}", name);

    let n_workflows = service.workflows.len();

    for workflow_index in 0..n_workflows {
        let (workflow_id, workflow) = service.workflows.iter().nth(workflow_index).unwrap();

        let n_tasks = match &workflow.submit {
            Submit::EthereumContract(EthereumContractSubmission { .. }) => 1,
            Submit::Aggregator { .. } => 3, // TODO: make sure this value will work as aggregator is updated to calculate quorum, power, etc.
            Submit::None => 1,
        };

        for task_number in 1..=n_tasks {
            let is_final = task_number == n_tasks;

            let (trigger_id, signed_data) = add_task(
                &clients.cli_ctx,
                service_id.clone(),
                Some(workflow_id.to_string()),
                get_input_for_service(name, service, configs, workflow_index),
                if is_final {
                    Some(std::time::Duration::from_secs(30))
                } else {
                    None
                },
            )
            .await?;

            if is_final {
                let signed_data = signed_data.context("no signed data returned")?;
                verify_signed_data(clients, name, signed_data, service, configs, workflow_index)
                    .await?;
            }

            if services.len() > 1 {
                // sanity check that all our services are for the same trigger
                for additional_service in &services[1..] {
                    tracing::info!("Testing Additional service for same trigger...");
                    assert_eq!(
                        additional_service
                            .workflows
                            .values()
                            .map(|w| w.trigger.clone())
                            .collect::<Vec<_>>(),
                        service
                            .workflows
                            .values()
                            .map(|w| w.trigger.clone())
                            .collect::<Vec<_>>(),
                    );

                    let service = clients
                        .cli_ctx
                        .deployment
                        .lock()
                        .unwrap()
                        .services
                        .get(&additional_service.id)
                        .unwrap()
                        .clone();

                    let workflow = service.workflows.get(workflow_id).unwrap().clone();

                    let chain_and_addr = match workflow.submit {
                        Submit::None => None,
                        Submit::EthereumContract(EthereumContractSubmission {
                            chain_name,
                            address,
                            ..
                        }) => Some((chain_name, address)),
                        Submit::Aggregator { url: _ } => Some((
                            service.manager.chain_name().clone(),
                            service.manager.eth_address_unchecked(),
                        )),
                    };

                    if let Some((chain_name, address)) = chain_and_addr {
                        let signed_data = wait_for_task_to_land(
                            &clients.cli_ctx,
                            &chain_name,
                            address,
                            trigger_id,
                            std::time::Duration::from_secs(10),
                            // we control the tests and *only* use on-chain triggers
                            // for multi-service tests
                            false,
                        )
                        .await?;

                        verify_signed_data(
                            clients,
                            name,
                            signed_data,
                            &service,
                            configs,
                            workflow_index,
                        )
                        .await?;
                    }
                }
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
) -> Option<Vec<u8>> {
    let permissions_req = || {
        PermissionsRequest {
            get_url: "https://postman-echo.com/get".to_string(),
            post_url: "https://postman-echo.com/post".to_string(),
            post_data: ("hello".to_string(), "world".to_string()),
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
            EthService::MultiTrigger => b"tttrrrrriiiigggeerrr".to_vec(),
            EthService::CronInterval | EthService::BlockInterval => Vec::new(),
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
            CosmosService::CronInterval | CosmosService::BlockInterval => Vec::new(),
        },
        AnyService::CrossChain(name) => match name {
            CrossChainService::CosmosToEthEchoData => b"hello eth world from cosmos".to_vec(),
        },
    };

    if input_data.is_empty() {
        None
    } else {
        Some(input_data)
    }
}

async fn verify_signed_data(
    clients: &Clients,
    name: AnyService,
    signed_data: SignedData,
    service: &Service,
    configs: &Configs,
    workflow_index: usize,
) -> Result<()> {
    let data = &signed_data.data;

    let input_req = || {
        get_input_for_service(name, service, configs, workflow_index)
            .expect("expected input data to be present for this test")
    };

    let expected_data = match name {
        AnyService::Eth(eth_name) => match eth_name {
            // Just echo
            EthService::EchoData
            | EthService::EchoDataSecondaryChain
            | EthService::EchoDataAggregator
            | EthService::MultiTrigger
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
            EthService::BlockInterval => Some(b"block-interval data".to_vec()),
            EthService::CronInterval => Some(b"cron-interval data".to_vec()),
        },
        AnyService::Cosmos(cosmos_name) => match cosmos_name {
            CosmosService::EchoData | CosmosService::ChainTriggerLookup => Some(input_req()),

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
            CosmosService::BlockInterval => Some(b"block-interval data".to_vec()),
            CosmosService::CronInterval => Some(b"cron-interval data".to_vec()),
        },
        AnyService::CrossChain(crosschain_name) => match crosschain_name {
            CrossChainService::CosmosToEthEchoData => Some(input_req()),
        },
    };

    // in some cases we just verify that we could deserialize the data
    // in others, we know what we expect exactly, make sure we got it
    if let Some(expected_data) = expected_data {
        assert_eq!(*data, expected_data);

        if let Ok(msg) = String::from_utf8(data.clone()) {
            tracing::info!("Response: {}", msg);
        }
    }

    let signing_key = clients
        .http_client
        .get_service_key(service.id.clone())
        .await?;

    // TODO - re-use stuff from https://github.com/Lay3rLabs/WAVS/pull/496 when it lands

    match signing_key {
        SigningKeyResponse::Secp256k1(bytes) => {
            let private_key = SigningKey::from_slice(&bytes)?;
            let service_address = alloy::primitives::Address::from_private_key(&private_key);

            let signature =
                alloy::primitives::PrimitiveSignature::from_raw(&signed_data.signature)?;

            let envelope_bytes = signed_data.envelope.abi_encode();
            let envelope_hash = eip191_hash_message(keccak256(&envelope_bytes));

            let signer_address = signature.recover_address_from_prehash(&envelope_hash)?;

            if service_address != signer_address {
                return Err(anyhow::anyhow!(
                    "Signature does not match service address: {} != {}",
                    service_address,
                    signer_address
                ));
            }
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
    pub get_url: String,
    pub post_url: String,
    pub post_data: (String, String),
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
