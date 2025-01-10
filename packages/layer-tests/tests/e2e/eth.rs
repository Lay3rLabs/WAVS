use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use alloy::node_bindings::AnvilInstance;
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use utils::{
    avs_client::AvsClientBuilder,
    eigen_client::{CoreAVSAddresses, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
    example_client::{SimpleSubmitClient, SimpleTriggerClient},
};
use wavs::{
    apis::{
        dispatcher::{ComponentWorld, Submit},
        trigger::Trigger,
    },
    config::Config,
};

use crate::e2e::payload::{CosmosQueryRequest, CosmosQueryResponse, SquareRequest, SquareResponse};

use super::{http::HttpClient, Digests, ServiceIds};

#[allow(dead_code)]
pub struct EthTestApp {
    pub eigen_client: EigenClient,
    pub core_contracts: CoreAVSAddresses,
    anvil: AnvilInstance,
}

impl EthTestApp {
    pub async fn new(_config: Config, anvil: AnvilInstance) -> Self {
        let config = EthClientConfig {
            ws_endpoint: Some(anvil.ws_endpoint().to_string()),
            http_endpoint: anvil.endpoint().to_string(),
            mnemonic: Some(
                "test test test test test test test test test test test junk".to_string(),
            ),
            hd_index: None,
            transport: None,
        };

        tracing::info!("Creating eth client on: {:?}", config.ws_endpoint);

        let eth_client = EthClientBuilder::new(config).build_signing().await.unwrap();
        let eigen_client = EigenClient::new(eth_client);

        let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();
        eigen_client
            .register_operator(&core_contracts)
            .await
            .unwrap();

        Self {
            eigen_client,
            anvil,
            core_contracts,
        }
    }

    pub async fn deploy_service_contracts(&self) -> (SimpleTriggerClient, SimpleSubmitClient) {
        let avs_client = AvsClientBuilder::new(self.eigen_client.eth.clone())
            .core_addresses(self.core_contracts.clone())
            .build(SimpleSubmitClient::deploy)
            .await
            .unwrap();

        avs_client
            .register_operator(&mut rand::rngs::OsRng)
            .await
            .unwrap();

        let submit_client =
            SimpleSubmitClient::new(avs_client.eth.clone(), avs_client.layer.service_manager);

        let trigger_client = SimpleTriggerClient::new_deploy(avs_client.eth.clone())
            .await
            .unwrap();

        (trigger_client, submit_client)
    }
}

pub async fn run_tests_ethereum(
    chain_name: String,
    chain_name2: String,
    anvil: AnvilInstance,
    anvil2: AnvilInstance,
    http_client: HttpClient,
    config: Config,
    digests: Digests,
    service_ids: ServiceIds,
) {
    tracing::info!("Running e2e ethereum tests");

    let app = EthTestApp::new(config.clone(), anvil).await;
    let app_2 = EthTestApp::new(config.clone(), anvil2).await;

    let trigger_echo_digest = digests.eth_trigger_echo_digest().await;
    let trigger_square_digest = digests.eth_trigger_square_digest().await;
    let cosmos_query_digest = digests.eth_cosmos_query().await;

    let trigger_echo_service_id_1 = service_ids.eth_trigger_echo_1();
    let trigger_echo_service_id_2 = service_ids.eth_trigger_echo_2();
    let trigger_echo_aggregate_service_id = service_ids.eth_trigger_echo_aggregate();
    let trigger_square_service_id = service_ids.eth_trigger_square();
    let cosmos_query_service_id = service_ids.eth_cosmos_query();

    let mut clients = HashMap::new();
    let mut contract_addrs = HashSet::new();

    for (service_id, digest, world, is_aggregate, is_second_ethereum) in [
        (
            trigger_echo_service_id_1.clone(),
            trigger_echo_digest.clone(),
            ComponentWorld::ChainEvent,
            false,
            false,
        ),
        (
            trigger_echo_service_id_2.clone(),
            trigger_echo_digest.clone(),
            ComponentWorld::ChainEvent,
            false,
            true,
        ),
        (
            trigger_echo_aggregate_service_id.clone(),
            trigger_echo_digest,
            ComponentWorld::ChainEvent,
            true,
            false,
        ),
        (
            trigger_square_service_id.clone(),
            trigger_square_digest,
            ComponentWorld::ChainEvent,
            false,
            false,
        ),
        (
            cosmos_query_service_id.clone(),
            cosmos_query_digest,
            ComponentWorld::ChainEvent,
            false,
            false,
        ),
    ] {
        if service_id.is_some() {
            let service_id = service_id.unwrap();
            let digest = digest.unwrap();

            let (trigger_client, submit_client) = match is_second_ethereum {
                false => app.deploy_service_contracts().await,
                true => app_2.deploy_service_contracts().await,
            };

            let chain_name = match is_second_ethereum {
                false => chain_name.clone(),
                true => chain_name2.clone(),
            };

            let app_name = match is_second_ethereum {
                false => "app",
                true => "app_2",
            };

            if !contract_addrs.insert((app_name, trigger_client.contract_address.clone())) {
                panic!(
                    "({app_name}) ({service_id}) Duplicate trigger contract address: {}",
                    trigger_client.contract_address
                );
            }
            if !contract_addrs.insert((app_name, submit_client.contract_address.clone())) {
                panic!(
                    "({app_name}) ({service_id}) Duplicate submit contract address: {}",
                    submit_client.contract_address
                );
            }

            let trigger_contract_address =
                Address::Eth(AddrEth::new_vec(trigger_client.contract_address.to_vec()).unwrap());
            let submit_contract_address =
                Address::Eth(AddrEth::new_vec(submit_client.contract_address.to_vec()).unwrap());

            http_client
                .create_service(
                    service_id.clone(),
                    digest,
                    Trigger::contract_event(trigger_contract_address.clone(), chain_name.clone()),
                    Submit::eigen_contract(
                        chain_name.to_string(),
                        submit_contract_address.clone(),
                        false, // FIXME, use is_aggregate: https://github.com/Lay3rLabs/WAVS/issues/254
                    ),
                    world,
                )
                .await
                .unwrap();

            tracing::info!("Service created: {}", service_id);

            if is_aggregate {
                http_client
                    .register_service_on_aggregator(
                        &chain_name,
                        submit_client.contract_address.clone(),
                        &config,
                    )
                    .await
                    .unwrap();
            }

            if clients
                .insert(service_id.clone(), (trigger_client, submit_client))
                .is_some()
            {
                panic!("Duplicate service id: {}", service_id);
            }
        }
    }

    if let Some(service_id) = trigger_echo_service_id_1 {
        let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
        tracing::info!("Submitting trigger_echo task...");
        let echo_trigger_id = trigger_client.add_trigger(b"foo".to_vec()).await.unwrap();

        tokio::time::timeout(Duration::from_secs(10), {
            let submit_client = submit_client.clone();
            async move {
                loop {
                    if submit_client.trigger_validated(echo_trigger_id).await {
                        break;
                    } else {
                        tracing::info!(
                            "Waiting on response for service {}, trigger {}",
                            service_id,
                            echo_trigger_id
                        );
                    }
                    // still open, waiting...
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        })
        .await
        .unwrap();
    }

    if let Some(service_id) = trigger_echo_service_id_2 {
        let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
        tracing::info!("Submitting trigger_echo task...");
        let echo_trigger_id = trigger_client.add_trigger(b"foo".to_vec()).await.unwrap();

        tokio::time::timeout(Duration::from_secs(10), {
            let submit_client = submit_client.clone();
            async move {
                loop {
                    if submit_client.trigger_validated(echo_trigger_id).await {
                        break;
                    } else {
                        tracing::info!(
                            "Waiting on response for service {}, trigger {}",
                            service_id,
                            echo_trigger_id
                        );
                    }
                    // still open, waiting...
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        })
        .await
        .unwrap();
    }

    if let Some(service_id) = trigger_square_service_id {
        let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
        tracing::info!("Submitting square task...");
        let square_trigger_id = trigger_client
            .add_trigger(serde_json::to_vec(&SquareRequest { x: 3 }).unwrap())
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(10), {
            let submit_client = submit_client.clone();
            async move {
                loop {
                    let data = submit_client.trigger_data(square_trigger_id).await.ok();

                    match data {
                        Some(data) => {
                            println!("{:?}", data);
                            let response = serde_json::from_slice::<SquareResponse>(&data).unwrap();

                            tracing::info!("GOT THE RESPONSE!");
                            tracing::info!("{:?}", response);
                            break;
                        }
                        None => {
                            tracing::info!(
                                "Waiting on response for service {}, trigger {}",
                                service_id,
                                square_trigger_id
                            );
                        }
                    }
                    // still open, waiting...
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        })
        .await
        .unwrap();
    }

    if let Some(service_id) = trigger_echo_aggregate_service_id {
        let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
        let echo_aggregate_trigger_id_1 = trigger_client
            .add_trigger(b"foo-aggregate".to_vec())
            .await
            .unwrap();

        let echo_aggregate_trigger_id_2 = trigger_client
            .add_trigger(b"bar-aggregate".to_vec())
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(10), {
            let submit_client = submit_client.clone();
            async move {
                loop {
                    let signature_1 = submit_client
                        .trigger_data(echo_aggregate_trigger_id_1)
                        .await
                        .ok();

                    let signature_2 = submit_client
                        .trigger_data(echo_aggregate_trigger_id_2)
                        .await
                        .ok();

                    match (signature_1, signature_2) {
                        (Some(signature_1), Some(signature_2)) => {
                            tracing::info!("GOT THE AGGREGATED SIGNATURES!",);
                            tracing::info!("1: {}", hex::encode(signature_1));
                            tracing::info!("2: {}", hex::encode(signature_2));
                            break;
                        }
                        (None, Some(_)) => {
                            tracing::info!("Got aggregation #1, waiting for #2...");
                        }
                        (Some(_), None) => {
                            tracing::info!("Got aggregation #2, waiting for #1...");
                        }
                        (None, None) => {
                            tracing::info!("Waiting for aggregation responses...");
                        }
                    }
                    // still open, waiting...
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        })
        .await
        .unwrap();
    }

    if let Some(service_id) = cosmos_query_service_id {
        let (trigger_client, submit_client) = clients.get(&service_id).unwrap();
        tracing::info!("Submitting cosmos query tasks...");
        let trigger_id = trigger_client
            .add_trigger(serde_json::to_vec(&CosmosQueryRequest::BlockHeight).unwrap())
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(10), {
            let submit_client = submit_client.clone();
            let service_id = service_id.clone();
            async move {
                loop {
                    let data = submit_client.trigger_data(trigger_id).await.ok();
                    match data {
                        Some(data) => {
                            let response =
                                serde_json::from_slice::<CosmosQueryResponse>(&data).unwrap();

                            tracing::info!("GOT THE RESPONSE!");
                            match response {
                                CosmosQueryResponse::BlockHeight(height) => {
                                    tracing::info!("height: {}", height);
                                }
                                _ => panic!("Expected block height"),
                            }

                            break;
                        }
                        None => {
                            tracing::info!(
                                "Waiting on response for service {}, trigger {}",
                                service_id,
                                trigger_id
                            );
                        }
                    }
                    // still open, waiting...
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        })
        .await
        .unwrap();

        let trigger_id = trigger_client
            .add_trigger(
                serde_json::to_vec(&CosmosQueryRequest::Balance {
                    // this test expects that we're running on Starship
                    // https://github.com/cosmology-tech/starship/blob/5635e853ac9e364f0ae9c87646536c30b6519748/starship/charts/devnet/configs/keys.json#L27
                    address: Address::new_cosmos_string(
                        "osmo1pss7nxeh3f9md2vuxku8q99femnwdjtc8ws4un",
                        None,
                    )
                    .unwrap(),
                })
                .unwrap(),
            )
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(10), {
            let submit_client = submit_client.clone();
            async move {
                loop {
                    let data = submit_client.trigger_data(trigger_id).await.ok();
                    match data {
                        Some(data) => {
                            let response =
                                serde_json::from_slice::<CosmosQueryResponse>(&data).unwrap();

                            tracing::info!("GOT THE RESPONSE!");
                            match response {
                                CosmosQueryResponse::Balance(balance) => {
                                    tracing::info!("balance: {}", balance);
                                }
                                _ => panic!("Expected balance"),
                            }

                            break;
                        }
                        None => {
                            tracing::info!(
                                "Waiting on response for service {}, trigger {}",
                                service_id,
                                trigger_id
                            );
                        }
                    }
                    // still open, waiting...
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        })
        .await
        .unwrap();
    }
}
