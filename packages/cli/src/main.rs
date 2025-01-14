mod args;
mod client;
mod config;
mod context;
mod deploy;
mod display;
mod exec;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use args::{CliSubmitKind, CliTriggerKind, Command};
use clap::Parser;
use client::HttpClient;
use context::ChainContext;
use deploy::{ServiceInfo, ServiceSubmitInfo, ServiceTriggerInfo};
use display::DisplayBuilder;
use exec::{exec_component, ExecComponentResponse};
use rand::rngs::OsRng;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utils::{
    avs_client::{AvsClientDeployer, SignedData},
    config::ConfigExt,
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_eth_client::{SimpleEthSubmitClient, SimpleEthTriggerClient, TriggerId},
};
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

    let mut display = DisplayBuilder::new();

    let mut ctx = ChainContext::try_new(&command, config).await;

    match command {
        Command::DeployEigenCore {
            register_operator,
            chain,
            args: _,
        } => {
            let eigen_client = ctx.get_eth_client(&chain);

            let core_contracts = match ctx.deployment.eigen_core.get(&chain) {
                Some(core_contracts) => {
                    match ctx
                        .address_exists_on_chain(&chain, core_contracts.delegation_manager.into())
                        .await
                    {
                        true => {
                            tracing::warn!("Core contracts already deployed for chain {}", chain);
                            Some(core_contracts.clone())
                        }
                        false => {
                            tracing::warn!("Core contracts already deployed for chain {}, but service manager not found.. redeploying", chain);
                            None
                        }
                    }
                }
                None => None,
            };

            let core_contracts = match core_contracts {
                Some(core_contracts) => core_contracts,
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

            ctx.deployment
                .eigen_core
                .insert(chain, core_contracts.clone());

            ctx.save_deployment();

            display.core_contracts = Some(core_contracts);
        }

        Command::DeployService {
            trigger_chain,
            trigger,
            cosmos_trigger_code_id,
            submit_chain,
            submit,
            register_operator,
            component,
            service_config,
            world,
            aggregate,
            args: _,
        } => {
            let trigger_info: ServiceTriggerInfo = match trigger {
                CliTriggerKind::SimpleEthContract => {
                    let chain_name = trigger_chain.expect("must have trigger chain for contract");

                    let address = SimpleEthTriggerClient::deploy(
                        ctx.get_eth_client(&chain_name).eth.provider.clone(),
                    )
                    .await
                    .unwrap();

                    ServiceTriggerInfo::EthSimpleContract {
                        chain_name,
                        address: address.into(),
                    }
                }
                CliTriggerKind::SimpleCosmosContract => {
                    let chain_name = trigger_chain.expect("must have trigger chain for contract");

                    let signing_client = ctx.get_cosmos_client(&chain_name);

                    let code_id = match cosmos_trigger_code_id {
                        Some(code_id) => code_id,
                        None => {
                            let path_to_wasm = workspace_path()
                                .join("examples")
                                .join("build")
                                .join("contracts")
                                .join("simple_example.wasm");

                            let wasm_byte_code = std::fs::read(path_to_wasm).unwrap();

                            let (code_id, _) = signing_client
                                .contract_upload_file(wasm_byte_code, None)
                                .await
                                .unwrap();

                            code_id
                        }
                    };

                    let address = SimpleCosmosTriggerClient::new_code_id(signing_client, code_id)
                        .await
                        .unwrap()
                        .contract_address;

                    ServiceTriggerInfo::CosmosSimpleContract {
                        chain_name,
                        address,
                    }
                }
            };

            let submit_info: ServiceSubmitInfo = match submit {
                CliSubmitKind::SimpleEthContract => {
                    let chain_name = submit_chain.expect("must have submit chain for contract");

                    let core_contracts = match ctx.deployment.eigen_core.get(&chain_name) {
                        Some(core_contracts) => core_contracts.clone(),
                        None => {
                            tracing::error!(
                                "Eigenlayer core contracts not deployed for chain {}, deploy those first!",
                                chain_name
                            );
                            return;
                        }
                    };

                    let eth_client = ctx.get_eth_client(&chain_name);

                    let avs_client = AvsClientDeployer::new(eth_client.eth)
                        .core_addresses(core_contracts)
                        .deploy(SimpleEthSubmitClient::deploy)
                        .await
                        .unwrap();

                    if register_operator {
                        avs_client.register_operator(&mut OsRng).await.unwrap();
                    }

                    ServiceSubmitInfo::EigenLayer {
                        chain_name,
                        avs_addresses: avs_client.layer,
                    }
                }
            };

            let service_info = ServiceInfo {
                trigger: trigger_info,
                submit: submit_info,
            };

            let http_client = HttpClient::new(&ctx.config);

            let wasm_bytes = read_component(component);
            let digest = http_client.upload_component(wasm_bytes).await;

            let (service_id, workflow_id) = http_client
                .create_service(
                    service_info.clone(),
                    aggregate,
                    digest,
                    service_config.unwrap_or_default(),
                    world,
                )
                .await;

            let mut workflows = HashMap::new();
            workflows.insert(workflow_id.clone(), service_info.clone());

            ctx.deployment
                .services
                .insert(service_id.clone(), workflows);

            ctx.save_deployment();

            display.service_info = Some(service_info);
        }

        Command::AddTask {
            service_id,
            workflow_id,
            input,
            args: _,
        } => {
            let input = decode_input(&input);

            let service_id = ServiceID::new(service_id).unwrap();
            let workflow_id = match workflow_id {
                Some(workflow_id) => WorkflowID::new(workflow_id).unwrap(),
                None => WorkflowID::new("default").unwrap(),
            };

            let service = match ctx.deployment.services.get(&service_id) {
                Some(workflows) => match workflows.get(&workflow_id) {
                    Some(service) => service.clone(),
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

            let trigger_id = match service.trigger {
                ServiceTriggerInfo::EthSimpleContract {
                    chain_name,
                    address,
                } => {
                    let client = SimpleEthTriggerClient::new(
                        ctx.get_eth_client(&chain_name).eth,
                        address.try_into().unwrap(),
                    );
                    client.add_trigger(input).await.unwrap()
                }
                ServiceTriggerInfo::CosmosSimpleContract {
                    chain_name,
                    address,
                } => {
                    let client =
                        SimpleCosmosTriggerClient::new(ctx.get_cosmos_client(&chain_name), address);
                    let trigger_id = client.add_trigger(input).await.unwrap();
                    TriggerId::new(trigger_id.u64())
                }
            };

            match service.submit {
                ServiceSubmitInfo::EigenLayer {
                    chain_name,
                    avs_addresses,
                } => {
                    let submit_client = SimpleEthSubmitClient::new(
                        ctx.get_eth_client(&chain_name).eth,
                        avs_addresses.service_manager,
                    );

                    tokio::time::timeout(Duration::from_secs(10), async move {
                        loop {
                            match submit_client.trigger_validated(trigger_id).await {
                                true => {
                                    let data =
                                        submit_client.trigger_data(trigger_id).await.unwrap();

                                    let signature =
                                        submit_client.trigger_signature(trigger_id).await.unwrap();

                                    return SignedData { data, signature };
                                }
                                false => {
                                    tracing::info!("Waiting for task response on {}", trigger_id);
                                }
                            }
                            // still open, waiting...
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                    })
                    .await
                    .unwrap();
                }
            }
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

fn workspace_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
