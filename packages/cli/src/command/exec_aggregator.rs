use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use utils::config::WAVS_ENV_PREFIX;
use wavs_engine::worlds::instance::{HostComponentLogger, InstanceData, InstanceDepsBuilder};
use wavs_types::{
    AggregatorAction, AggregatorInput, AllowedHostPermission, Component, ComponentDigest,
    ComponentSource, Permissions, Service, ServiceManager, ServiceStatus, SignatureKind, Submit,
    Trigger, TriggerAction, TriggerConfig, WasmResponse, Workflow, WorkflowId,
};

use crate::util::read_component;

fn create_dummy_service(
    source: ComponentSource,
    env_keys: BTreeSet<String>,
    config: BTreeMap<String, String>,
    fuel_limit: Option<u64>,
    time_limit_seconds: Option<u64>,
) -> Service {
    let component = Component {
        source,
        permissions: Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
            raw_sockets: true,
            dns_resolution: true,
        },
        fuel_limit,
        time_limit_seconds,
        config,
        env_keys,
    };
    Service {
        name: "dummy-service".to_string(),
        workflows: [(
            WorkflowId::default(),
            Workflow {
                trigger: Trigger::Manual,
                component: component.clone(),
                submit: Submit::Aggregator {
                    component: Box::new(component),
                    signature_kind: SignatureKind::evm_default(),
                },
            },
        )]
        .into(),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: "evm:dummy".parse().unwrap(),
            address: alloy_primitives::Address::ZERO,
        },
    }
}
fn create_dummy_input(service: &Service) -> AggregatorInput {
    AggregatorInput {
        trigger_action: TriggerAction {
            config: TriggerConfig {
                service_id: service.id(),
                workflow_id: service.workflows.keys().next().cloned().unwrap(),
                trigger: service.workflows.values().next().unwrap().trigger.clone(),
            },
            data: wavs_types::TriggerData::default(),
        },
        operator_response: WasmResponse {
            event_id_salt: None,
            ordering: None,
            payload: vec![],
        },
    }
}

pub struct ExecAggregator;

pub struct ExecAggregatorArgs {
    pub component: String,
    pub input: Option<String>,
    pub fuel_limit: Option<u64>,
    pub time_limit: Option<u64>,
    pub config: BTreeMap<String, String>,
}

impl ExecAggregator {
    pub async fn run(
        cli_config: &crate::config::Config,
        ExecAggregatorArgs {
            component,
            input,
            fuel_limit,
            time_limit,
            config,
        }: ExecAggregatorArgs,
    ) -> Result<ExecAggregatorResult> {
        let component_path = component;

        tracing::info!(
            "Executing packet with aggregator component: {}",
            component_path
        );

        let wasm_bytes = read_component(&component_path)?;

        let env_keys = std::env::vars()
            .map(|(key, _)| key)
            .filter(|key| key.starts_with(WAVS_ENV_PREFIX))
            .collect();

        let source = ComponentSource::Digest(ComponentDigest::hash(&wasm_bytes));
        let service = create_dummy_service(source, env_keys, config, fuel_limit, time_limit);

        // Read input from file or create a dummy one
        let input: AggregatorInput = if let Some(input_path) = input {
            let input_json = std::fs::read_to_string(&input_path)?;
            serde_json::from_str(&input_json)?
        } else {
            create_dummy_input(&service)
        };

        let mut wt_config = wasmtime::Config::new();
        wt_config.wasm_component_model(true);
        wt_config.async_support(true);
        wt_config.consume_fuel(true);
        let engine = wasmtime::Engine::new(&wt_config)?;

        let mut instance_deps = InstanceDepsBuilder {
            component: wasmtime::component::Component::new(&engine, &wasm_bytes)?,
            workflow_id: input.trigger_action.config.workflow_id.clone(),
            data: InstanceData::new_aggregator(input.event_id().unwrap()),

            engine: &engine,
            data_dir: tempfile::tempdir()?.keep(),
            chain_configs: &cli_config.chains.read().unwrap(),
            log: HostComponentLogger::AggregatorHostComponentLogger(
                |_service_id, _workflow_id, _digest, level, message| {
                    match level {
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Error => {
                    tracing::error!("{}", message)
                }
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Warn => {
                    tracing::warn!("{}", message)
                }
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Info => {
                    tracing::info!("{}", message)
                }
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Debug => {
                    tracing::debug!("{}", message)
                }
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Trace => {
                    tracing::trace!("{}", message)
                }
            }
                },
            ),
            keyvalue_ctx: wavs_engine::backend::wasi_keyvalue::context::KeyValueCtx::new(
                utils::storage::db::WavsDb::new()?,
                service.id().to_string(),
            ),
            service,
        }
        .build()?;

        let initial_fuel = instance_deps.store.get_fuel()?;
        let start_time = Instant::now();
        let actions =
            wavs_engine::worlds::aggregator::execute::execute_input(&mut instance_deps, input)
                .await?;
        let fuel_used = initial_fuel - instance_deps.store.get_fuel()?;
        let time_elapsed = start_time.elapsed().as_millis();

        Ok(ExecAggregatorResult::Packet {
            actions: actions
                .into_iter()
                .map(wavs_types::AggregatorAction::try_from)
                .collect::<Result<Vec<AggregatorAction>>>()?,
            fuel_used,
            time_elapsed,
        })
    }
}

#[derive(serde::Serialize)]
pub enum ExecAggregatorResult {
    Packet {
        actions: Vec<AggregatorAction>,
        fuel_used: u64,
        time_elapsed: u128,
    },
}

impl std::fmt::Display for ExecAggregatorResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecAggregatorResult::Packet {
                actions,
                fuel_used,
                time_elapsed,
            } => {
                write!(f, "Fuel used: \n{}", fuel_used)?;
                if *time_elapsed > 0 {
                    write!(f, "\n\nTime elapsed (ms): \n{}", time_elapsed)?;
                }
                writeln!(f, "\n\nPacket execution completed")?;
                writeln!(f, "Actions generated: {}", actions.len())?;
                for (i, action) in actions.iter().enumerate() {
                    writeln!(f, "  Action {}: {:?}", i + 1, action)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use layer_climb::prelude::EvmAddr;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use utils::filesystem::workspace_path;
    use wavs_types::{
        AllowedHostPermission, EvmChainConfig, Service, ServiceManager, ServiceStatus, Submit,
        Trigger, Workflow, WorkflowId,
    };

    fn create_test_service(component_path: &str) -> Service {
        let wasm_bytes = read_component(component_path).unwrap();
        let digest = wavs_types::ComponentDigest::hash(&wasm_bytes);

        let component = Component {
            source: ComponentSource::Digest(digest),
            permissions: Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
                raw_sockets: true,
                dns_resolution: true,
            },
            fuel_limit: None,
            time_limit_seconds: None,
            config: [
                ("chain".to_string(), "evm:31337".to_string()),
                (
                    "service_handler".to_string(),
                    "0x0000000000000000000000000000000000000000".to_string(),
                ),
            ]
            .into_iter()
            .collect(),
            env_keys: Default::default(),
        };
        Service {
            name: "test-service".to_string(),
            workflows: [(
                WorkflowId::default(),
                Workflow {
                    trigger: Trigger::Manual,
                    component: component.clone(),
                    submit: Submit::Aggregator {
                        component: Box::new(component),
                        signature_kind: SignatureKind::evm_default(),
                    },
                },
            )]
            .into(),
            status: ServiceStatus::Active,
            manager: ServiceManager::Evm {
                chain: "evm:anvil".parse().unwrap(),
                address: alloy_primitives::Address::ZERO,
            },
        }
    }

    fn create_test_input(service: &Service) -> AggregatorInput {
        AggregatorInput {
            trigger_action: wavs_types::TriggerAction {
                config: wavs_types::TriggerConfig {
                    service_id: service.id(),
                    workflow_id: service.workflows.keys().next().cloned().unwrap(),
                    trigger: service.workflows.values().next().unwrap().trigger.clone(),
                },
                data: wavs_types::TriggerData::default(),
            },
            operator_response: wavs_types::WasmResponse {
                event_id_salt: None,
                ordering: None,
                payload: b"test data".to_vec().into(),
            },
        }
    }

    #[tokio::test]
    async fn test_exec_aggregator_input() {
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("simple_aggregator.wasm")
            .to_string_lossy()
            .to_string();

        let service = create_test_service(&component_path);
        let input = create_test_input(&service);
        let mut input_file = NamedTempFile::new().unwrap();
        input_file
            .write_all(serde_json::to_string(&input).unwrap().as_bytes())
            .unwrap();

        let config = [
            ("chain".to_string(), "evm:31337".to_string()),
            (
                "service_handler".to_string(),
                "0x0000000000000000000000000000000000000000".to_string(),
            ),
        ]
        .into_iter()
        .collect();

        let args = ExecAggregatorArgs {
            component: component_path,
            input: Some(input_file.path().to_string_lossy().to_string()),
            fuel_limit: None,
            time_limit: None,
            config,
        };

        let config = crate::config::Config::default();
        config
            .chains
            .write()
            .unwrap()
            .add_chain(
                "evm:31337".to_string().parse().unwrap(),
                EvmChainConfig {
                    chain_id: "31337".to_string().parse().unwrap(),
                    http_endpoint: Some("http://localhost:8545".to_string()),
                    faucet_endpoint: None,
                    ws_endpoints: vec![],
                    ws_priority_endpoint_index: None,
                }
                .into(),
            )
            .unwrap();
        let result = ExecAggregator::run(&config, args).await.unwrap();

        match result {
            ExecAggregatorResult::Packet { actions, .. } => {
                assert_eq!(actions.len(), 1);
                match &actions[0] {
                    wavs_types::AggregatorAction::Submit(submit) => match submit {
                        wavs_types::SubmitAction::Evm(evm_submit) => {
                            assert_eq!(evm_submit.chain, "evm:31337".parse().unwrap());
                            assert_eq!(evm_submit.address, EvmAddr::new([0u8; 20]));
                        }
                        _ => panic!("Expected Evm Submit action, got {:?}", submit),
                    },
                    _ => panic!("Expected Submit action, got {:?}", actions[0]),
                }
            }
        }
    }

    #[tokio::test]
    async fn test_exec_aggregator_without_packet() {
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("simple_aggregator.wasm")
            .to_string_lossy()
            .to_string();

        let config = [
            ("chain".to_string(), "evm:31337".to_string()),
            (
                "service_handler".to_string(),
                "0x0000000000000000000000000000000000000000".to_string(),
            ),
        ]
        .into_iter()
        .collect();

        let args = ExecAggregatorArgs {
            component: component_path,
            input: None,
            fuel_limit: None,
            time_limit: None,
            config,
        };
        let config = crate::config::Config::default();
        config
            .chains
            .write()
            .unwrap()
            .add_chain(
                "evm:31337".to_string().parse().unwrap(),
                EvmChainConfig {
                    chain_id: "31337".to_string().parse().unwrap(),
                    http_endpoint: Some("http://localhost:8545".to_string()),
                    faucet_endpoint: None,
                    ws_endpoints: vec![],
                    ws_priority_endpoint_index: None,
                }
                .into(),
            )
            .unwrap();
        let result = ExecAggregator::run(&config, args).await.unwrap();

        match result {
            ExecAggregatorResult::Packet { actions, .. } => {
                assert_eq!(actions.len(), 1);
                match &actions[0] {
                    wavs_types::AggregatorAction::Submit(submit) => match submit {
                        wavs_types::SubmitAction::Evm(evm_submit) => {
                            assert_eq!(evm_submit.chain, "evm:31337".parse().unwrap());
                            assert_eq!(evm_submit.address, EvmAddr::new([0u8; 20]));
                        }
                        _ => panic!("Expected Evm Submit action, got {:?}", submit),
                    },
                    _ => panic!("Expected Submit action, got {:?}", actions[0]),
                }
            }
        }
    }
}
