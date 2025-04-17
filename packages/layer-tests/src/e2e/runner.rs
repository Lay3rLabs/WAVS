use std::{path::PathBuf, sync::Arc};

use alloy_provider::Provider;
use alloy_signer::{k256::ecdsa::SigningKey, Signature};
use alloy_signer_local::PrivateKeySigner;
use anyhow::Context;
use futures::{stream::FuturesUnordered, StreamExt};
use layer_climb::prelude::Address;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;
use utils::context::AppContext;
use wavs_types::{
    ChainName, EnvelopeSignature, EthereumContractSubmission, Service, SigningKeyResponse, Submit,
};

use crate::e2e::add_task::{add_task, wait_for_task_to_land};

use super::{
    add_task::SignedData,
    clients::Clients,
    config::Configs,
    matrix::{AnyService, CosmosService, CrossChainService, EthService},
    services::Services,
};

pub fn run_tests(ctx: AppContext, configs: Configs, clients: Clients, services: Services) {
    let all: Vec<(AnyService, (Service, Option<Service>))> = services.lookup.into_iter().collect();

    let configs = Arc::new(configs);
    let clients = Arc::new(clients);

    let mut serial_futures = Vec::new();
    let mut concurrent_futures = FuturesUnordered::new();

    for (name, (service, multi_trigger_service)) in all {
        let configs = configs.clone();
        let clients = clients.clone();
        let fut = async move {
            tracing::info!("Testing service: {:?}", name);
            let start_time = Instant::now();
            test_service(name, service, multi_trigger_service, &configs, &clients).await;
            tracing::info!(
                "Service {:?} passed (ran for {}ms)",
                name,
                start_time.elapsed().as_millis()
            );
        };

        if name.concurrent() {
            concurrent_futures.push(fut);
        } else {
            serial_futures.push(fut);
        }
    }

    ctx.rt.block_on(async move {
        tracing::info!("\n\n Running serial tests...");
        for fut in serial_futures {
            fut.await;
        }

        tracing::info!("\n\n Running concurrent tests...");
        while (concurrent_futures.next().await).is_some() {}
    });
}

async fn test_service(
    name: AnyService,
    service: Service,
    multi_trigger_service: Option<Service>,
    configs: &Configs,
    clients: &Clients,
) {
    let service_id = service.id.to_string();

    if let Some(multi_trigger_service) = &multi_trigger_service {
        // sanity checks for multi-trigger, to surface errors earlier
        // since mistakes here lead to hard-to-catch race conditions
        assert_eq!(name, AnyService::Eth(EthService::MultiTrigger));
        assert!(multi_trigger_service.workflows.len() == 1);
        assert!(service.workflows.len() == 1);

        let workflow = service.workflows.values().next().unwrap();
        let multi_trigger_workflow = multi_trigger_service.workflows.values().next().unwrap();

        // the trigger should be the same
        assert_eq!(multi_trigger_workflow.trigger, workflow.trigger);
        // but the submission should be different
        assert_ne!(multi_trigger_workflow.submit, workflow.submit);
        // if/when https://github.com/Lay3rLabs/WAVS/pull/502 lands (or any PR that has a distinct service manager per service)
        // then the service manager should be different too
        // assert_ne!(multi_trigger_service.manager, service.manager);
    }

    let n_workflows = service.workflows.len();

    for workflow_index in 0..n_workflows {
        let (workflow_id, workflow) = service.workflows.iter().nth(workflow_index).unwrap();

        let n_tasks = match &workflow.submit {
            Submit::EthereumContract(EthereumContractSubmission { .. }) => 1,
            Submit::Aggregator { .. } => 3, // TODO: make sure this value will work as aggregator is updated to calculate quorum, power, etc.
            Submit::None => 1,
        };

        let submit_client = clients.get_eth_client(service.manager.chain_name());
        let submit_start_block = submit_client.provider.get_block_number().await.unwrap();

        for task_number in 1..=n_tasks {
            let is_final = task_number == n_tasks;

            let (trigger_id, signed_data) = add_task(
                clients,
                service_id.clone(),
                Some(workflow_id.to_string()),
                get_input_for_service(name, &service, configs, workflow_index),
                submit_client.clone(),
                submit_start_block,
                is_final,
            )
            .await
            .unwrap();

            if is_final {
                let signed_data = signed_data.context("no signed data returned").unwrap();
                verify_signed_data(
                    clients,
                    name,
                    signed_data,
                    &service,
                    configs,
                    workflow_index,
                )
                .await;

                if let Some(multi_trigger_service) = &multi_trigger_service {
                    let multi_trigger_workflow =
                        multi_trigger_service.workflows.values().next().unwrap();

                    // we already triggered - now we just wait for it to land at the expected address
                    let address = match &multi_trigger_workflow.submit {
                        Submit::None => {
                            panic!("no submission in multi-trigger service");
                        }
                        Submit::EthereumContract(EthereumContractSubmission {
                            address, ..
                        }) => *address,
                        Submit::Aggregator { .. } => {
                            multi_trigger_service.manager.eth_address_unchecked()
                        }
                    };

                    // we control the tests and *only* use on-chain triggers
                    // for multi-service tests
                    let signed_data = wait_for_task_to_land(
                        submit_client.clone(),
                        address, // this is a different address than the original service submission
                        trigger_id,
                        submit_start_block,
                    )
                    .await;

                    verify_signed_data(
                        clients,
                        name,
                        signed_data,
                        multi_trigger_service,
                        configs,
                        0,
                    )
                    .await;
                }
            }
        }
    }
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
) {
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
                let resp: CosmosQueryResponse = serde_json::from_slice(data).unwrap();
                tracing::info!("Response: {:?}", resp);
                None
            }

            EthService::Permissions => {
                let resp: PermissionsResponse = serde_json::from_slice(data).unwrap();
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
                let resp: PermissionsResponse = serde_json::from_slice(data).unwrap();
                tracing::info!("Response: {:?}", resp);
                None
            }

            CosmosService::CosmosQuery => {
                let resp: CosmosQueryResponse = serde_json::from_slice(data).unwrap();
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

    let service_signing_key = clients
        .http_client
        .get_service_key(service.id.clone())
        .await
        .unwrap();

    match service_signing_key {
        SigningKeyResponse::Secp256k1(service_signing_key_bytes) => {
            let service_private_key = SigningKey::from_slice(&service_signing_key_bytes).unwrap();
            let service_signer = PrivateKeySigner::from_signing_key(service_private_key);
            let service_address = service_signer.address();

            let envelope_signature =
                EnvelopeSignature::Secp256k1(Signature::from_raw(&signed_data.signature).unwrap());
            let envelope_address = envelope_signature
                .eth_signer_address(&signed_data.envelope)
                .unwrap();

            if service_address != envelope_address {
                panic!(
                    "Signature does not match service {} address: {} (via service) != {} (via signature)",
                    service.id, service_address, envelope_address
                );
            }
        }
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
