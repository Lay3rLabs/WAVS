mod args;
mod client;
mod config;
mod context;
mod deploy;
mod display;
mod exec;
mod task;

use std::{collections::HashMap, path::Path};

use args::Command;
use clap::Parser;
use client::{get_avs_client, HttpClient};
use context::ChainContext;
use display::{DisplayBuilder, ServiceAndWorkflow};
use exec::{exec_component, ExecComponentResponse};
use rand::rngs::OsRng;
use task::add_task;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{config::ConfigExt, layer_contract_client::SignedData};
use utils::{ServiceID, WorkflowID};

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let command = Command::parse();
    let config = command.config();

    // setup tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .without_time()
                .with_target(false),
        )
        .with(config.tracing_env_filter().unwrap())
        .try_init()
        .unwrap();

    let mut display = DisplayBuilder::new();

    let ctx = ChainContext::try_new(&command, &config).await;

    match command {
        Command::DeployCore {
            register_operator, ..
        } => {
            let ChainContext {
                eigen_client,
                mut deployment,
            } = ctx.unwrap();

            let core_contracts = match deployment.eigen_core.get(&config.chain) {
                Some(core_contracts) => {
                    tracing::warn!("Core contracts already deployed for chain {}", config.chain);
                    core_contracts.clone()
                }
                None => {
                    let core_contracts = eigen_client.deploy_core_contracts().await.unwrap();

                    if register_operator {
                        eigen_client
                            .register_operator(&core_contracts)
                            .await
                            .unwrap();
                    }

                    core_contracts
                }
            };

            deployment
                .eigen_core
                .insert(config.chain.clone(), core_contracts.clone());
            deployment.save(&config).unwrap();

            display.core_contracts = Some(core_contracts);
        }

        Command::DeployService {
            register_operator,
            component,
            service_manager,
            service_config,
            ..
        } => {
            let ChainContext {
                eigen_client,
                mut deployment,
            } = ctx.unwrap();

            let core_contracts = match deployment.eigen_core.get(&config.chain) {
                Some(core_contracts) => core_contracts.clone(),
                None => {
                    tracing::error!(
                        "Core contracts not deployed for chain {}, deploy those first!",
                        config.chain
                    );
                    return;
                }
            };

            let avs_client =
                get_avs_client(&eigen_client, core_contracts.clone(), service_manager).await;

            if register_operator {
                avs_client.register_operator(&mut OsRng).await.unwrap();
            }

            let http_client = HttpClient::new(&config);

            let wasm_bytes = read_component(component);
            let digest = http_client.upload_component(wasm_bytes).await;

            let (service_id, workflow_id) = http_client
                .create_service(
                    avs_client.layer.trigger,
                    avs_client.layer.service_manager,
                    digest,
                    service_config.unwrap_or_default(),
                )
                .await;

            let mut workflows = HashMap::new();
            workflows.insert(workflow_id.clone(), avs_client.layer.clone());

            deployment
                .eth_services
                .insert(service_id.clone(), workflows);
            deployment.save(&config).unwrap();

            display.service = Some(ServiceAndWorkflow {
                service_id,
                workflow_id,
            });
            display.core_contracts = Some(core_contracts);
            display.layer_addresses = Some(avs_client.layer);
        }

        Command::AddTask {
            service_id,
            workflow_id,
            input,
            ..
        } => {
            let ChainContext {
                eigen_client,
                deployment,
            } = ctx.unwrap();

            let input = decode_input(&input);

            if !deployment.eigen_core.contains_key(&config.chain) {
                tracing::error!(
                    "Core contracts not deployed for chain {}, deploy those first!",
                    config.chain
                );
                return;
            };

            let service_id = ServiceID::new(service_id).unwrap();
            let workflow_id = match workflow_id {
                Some(workflow_id) => WorkflowID::new(workflow_id).unwrap(),
                None => WorkflowID::new("default").unwrap(),
            };

            let service_contracts = match deployment.eth_services.get(&service_id) {
                Some(workflow_contracts) => match workflow_contracts.get(&workflow_id) {
                    Some(service_contracts) => service_contracts.clone(),
                    None => {
                        tracing::error!(
                            "Service contracts not deployed for service {} and workflow {}, deploy those first!",
                            service_id,
                            workflow_id
                        );
                        return;
                    }
                },
                None => {
                    tracing::error!(
                        "Service contracts not deployed for service {}, deploy those first!",
                        service_id
                    );
                    return;
                }
            };

            let signed_data = add_task(
                eigen_client.eth,
                service_id.clone(),
                workflow_id.clone(),
                &service_contracts,
                input,
            )
            .await;

            display.layer_addresses = Some(service_contracts);
            display.service = Some(ServiceAndWorkflow {
                service_id,
                workflow_id,
            });
            display.signed_data = Some(signed_data);
        }

        Command::Exec {
            component, input, ..
        } => {
            let input_bytes = match input.starts_with('@') {
                true => {
                    let filepath = shellexpand::tilde(&input[1..]).to_string();

                    std::fs::read(filepath).unwrap()
                }

                false => {
                    if Path::new(&shellexpand::tilde(&input).to_string()).exists() {
                        tracing::warn!(
                            "Are you sure you didn't mean to use @ to specify file input?"
                        );
                    }

                    decode_input(&input)
                }
            };

            let wasm_bytes = read_component(component);

            let ExecComponentResponse {
                output_bytes,
                gas_used,
            } = exec_component(wasm_bytes, input_bytes).await;

            display.signed_data = Some(SignedData {
                data: output_bytes,
                signature: vec![],
            });

            display.gas_used = Some(gas_used);
        }
    }

    display.show();
}

fn decode_input(input: &str) -> Vec<u8> {
    if let Ok(bytes) = hex::decode(input) {
        bytes
    } else {
        let hex = input.as_bytes().iter().fold(String::new(), |mut acc, b| {
            acc.push_str(&format!("{:02x}", b));
            acc
        });
        hex::decode(hex).expect("Failed to decode input")
    }
}

fn read_component(path: impl AsRef<Path>) -> Vec<u8> {
    let path = if path.as_ref().is_absolute() {
        path.as_ref().to_path_buf()
    } else {
        // if relative path, parent (root of the repo) is relative 2 back from this file
        Path::new("../../").join(path.as_ref())
    };

    std::fs::read(path).unwrap()
}
