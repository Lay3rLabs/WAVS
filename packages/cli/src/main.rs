mod args;
mod client;
mod config;
mod deploy;
mod display;
mod task;

use std::collections::HashMap;

use args::Command;
use clap::Parser;
use client::{get_avs_client, get_eigen_client, HttpClient};
use deploy::Deployment;
use display::{DisplayBuilder, ServiceAndWorkflow};
use rand::rngs::OsRng;
use task::add_task;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::config::ConfigExt;
use wavs::apis::{ServiceID, WorkflowID};

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

    let eigen_client = get_eigen_client(&config).await;
    let http_client = HttpClient::new(&config);
    let mut deployment = Deployment::load(&config).unwrap();
    deployment
        .sanitize(&command, &config, &eigen_client)
        .await
        .unwrap();

    let mut display = DisplayBuilder::new();

    match command {
        Command::DeployCore {
            register_operator, ..
        } => {
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
            ..
        } => {
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

            let digest = http_client.upload_component(&component).await;

            let (service_id, workflow_id) = http_client
                .create_service(
                    avs_client.layer.trigger,
                    avs_client.layer.service_manager,
                    digest,
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
            let input = if let Ok(bytes) = hex::decode(input.clone()) {
                bytes
            } else {
                let hex = input.as_bytes().iter().fold(String::new(), |mut acc, b| {
                    acc.push_str(&format!("{:02x}", b));
                    acc
                });
                hex::decode(hex).expect("Failed to decode input")
            };

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
    }

    display.show();
}
