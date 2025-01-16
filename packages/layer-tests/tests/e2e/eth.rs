use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use alloy::{
    node_bindings::{Anvil, AnvilInstance},
    sol_types::SolEvent,
};
use layer_climb::prelude::*;
use serde::{Deserialize, Serialize};
use utils::{
    avs_client::AvsClientDeployer,
    config::EthereumChainConfig,
    eigen_client::{CoreAVSAddresses, EigenClient},
    eth_client::{EthClientBuilder, EthClientConfig},
    example_eth_client::{
        example_submit::SimpleSubmit, example_trigger::SimpleTrigger, SimpleEthSubmitClient,
        SimpleEthTriggerClient,
    },
};
use wavs::{
    apis::{dispatcher::Submit, trigger::Trigger},
    config::Config,
    AppContext,
};

use crate::e2e::payload::{CosmosQueryRequest, CosmosQueryResponse, SquareRequest, SquareResponse};

use super::{http::HttpClient, Digests, ServiceIds};

pub fn start_chain(
    ctx: AppContext,
    index: u8,
) -> (String, EthereumChainConfig, Option<AnvilInstance>) {
    let port = 8545 + index as u16;
    let chain_id = 31337 + index as u64;

    let anvil = Anvil::new().port(port).chain_id(chain_id).spawn();

    (
        format!("local-eth-test-{}", index),
        EthereumChainConfig {
            chain_id: 31337.to_string(),
            http_endpoint: anvil.endpoint(),
            ws_endpoint: anvil.ws_endpoint(),
            aggregator_endpoint: None,
            faucet_endpoint: None,
        },
        Some(anvil),
    )
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct EthTestApp {
    pub eigen_client: EigenClient,
    pub core_contracts: CoreAVSAddresses,
    pub chain_name: String,
    pub chain_config: EthereumChainConfig,
    anvil: Option<Arc<AnvilInstance>>,
}

impl EthTestApp {
    pub async fn new(
        chain_name: String,
        chain_config: EthereumChainConfig,
        anvil: Option<AnvilInstance>,
    ) -> Self {
        let config = EthClientConfig {
            ws_endpoint: Some(chain_config.ws_endpoint.clone()),
            http_endpoint: chain_config.http_endpoint.clone(),
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
            anvil: anvil.map(Arc::new),
            core_contracts,
            chain_name,
            chain_config,
        }
    }

    pub async fn deploy_service_contracts(
        &self,
    ) -> (SimpleEthTriggerClient, SimpleEthSubmitClient) {
        let avs_client = AvsClientDeployer::new(self.eigen_client.eth.clone())
            .core_addresses(self.core_contracts.clone())
            .deploy(SimpleEthSubmitClient::deploy)
            .await
            .unwrap();

        avs_client
            .register_operator(&mut rand::rngs::OsRng)
            .await
            .unwrap();

        let submit_client =
            SimpleEthSubmitClient::new(avs_client.eth.clone(), avs_client.layer.service_manager);

        let trigger_client = SimpleEthTriggerClient::new_deploy(avs_client.eth.clone())
            .await
            .unwrap();

        (trigger_client, submit_client)
    }
}

pub async fn run_tests(
    eth_apps: Vec<EthTestApp>,
    http_client: HttpClient,
    digests: Digests,
    service_ids: ServiceIds,
) {
    tracing::info!("Running e2e ethereum tests");

    let mut clients = HashMap::new();
    let mut contract_addrs = HashSet::new();

    for (service_id, digest, is_aggregate, app) in [
        (
            service_ids.eth_echo_1.clone(),
            digests.echo_data.clone(),
            false,
            eth_apps[0].clone(),
        ),
        (
            service_ids.eth_echo_2.clone(),
            digests.echo_data.clone(),
            false,
            eth_apps[1].clone(),
        ),
        (
            service_ids.eth_echo_aggregate.clone(),
            digests.echo_data.clone(),
            true,
            eth_apps[0].clone(),
        ),
        (
            service_ids.eth_square.clone(),
            digests.square.clone(),
            false,
            eth_apps[0].clone(),
        ),
        (
            service_ids.eth_cosmos_query.clone(),
            digests.cosmos_query.clone(),
            false,
            eth_apps[0].clone(),
        ),
    ] {
        if service_id.is_some() {
            let service_id = service_id.unwrap();
            let digest = digest.unwrap();

            let (trigger_client, submit_client) = app.deploy_service_contracts().await;

            if !contract_addrs.insert((
                app.chain_name.clone(),
                trigger_client.contract_address.clone(),
            )) {
                panic!(
                    "({}) ({}) Duplicate trigger contract address: {}",
                    app.chain_name, service_id, trigger_client.contract_address
                );
            }
            if !contract_addrs.insert((
                app.chain_name.clone(),
                submit_client.contract_address.clone(),
            )) {
                panic!(
                    "({}) ({}) Duplicate submit contract address: {}",
                    app.chain_name, service_id, submit_client.contract_address
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
                    Trigger::eth_contract_event(
                        trigger_contract_address.clone(),
                        app.chain_name.clone(),
                        SimpleTrigger::NewTrigger::SIGNATURE_HASH,
                    ),
                    Submit::eigen_contract(
                        app.chain_name.to_string(),
                        submit_contract_address.clone(),
                        false, // FIXME, use is_aggregate: https://github.com/Lay3rLabs/WAVS/issues/254
                    ),
                )
                .await
                .unwrap();

            tracing::info!("Service created: {}", service_id);

            if is_aggregate {
                http_client
                    .register_service_on_aggregator(
                        &app.chain_name,
                        submit_client.contract_address.clone(),
                        &app.chain_config,
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

    if let Some(service_id) = service_ids.eth_echo_1 {
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

    if let Some(service_id) = service_ids.eth_echo_2 {
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

    if let Some(service_id) = service_ids.eth_square {
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

    if let Some(service_id) = service_ids.eth_echo_aggregate {
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

    if let Some(service_id) = service_ids.eth_cosmos_query {
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
