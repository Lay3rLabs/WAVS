use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use utils::config::WAVS_ENV_PREFIX;
use wavs_engine::worlds::instance::{HostComponentLogger, InstanceDepsBuilder};
use wavs_types::{
    AggregatorAction, AllowedHostPermission, Component, ComponentDigest, ComponentSource, Envelope,
    EnvelopeSignature, Packet, Permissions, Service, ServiceManager, ServiceStatus, SignatureKind,
    Submit, Trigger, Workflow, WorkflowId,
};

use crate::util::read_component;

fn create_dummy_packet(
    digest: ComponentDigest,
    env_keys: BTreeSet<String>,
    config: BTreeMap<String, String>,
    fuel_limit: Option<u64>,
    time_limit_seconds: Option<u64>,
) -> Packet {
    let service = Service {
        name: "dummy-service".to_string(),
        workflows: [(
            WorkflowId::default(),
            Workflow {
                trigger: Trigger::Manual,
                component: Component {
                    source: ComponentSource::Digest(digest),
                    permissions: Permissions {
                        allowed_http_hosts: AllowedHostPermission::All,
                        file_system: true,
                    },
                    fuel_limit,
                    time_limit_seconds,
                    config,
                    env_keys,
                },
                submit: Submit::None,
            },
        )]
        .into(),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: "evm:dummy".parse().unwrap(),
            address: alloy_primitives::Address::ZERO,
        },
    };

    Packet {
        envelope: Envelope {
            eventId: [0u8; 20].into(),
            ordering: [0u8; 12].into(),
            payload: vec![].into(),
        },
        workflow_id: WorkflowId::default(),
        service,
        signature: EnvelopeSignature {
            data: alloy_primitives::Signature::from_bytes_and_parity(&[0u8; 64], false).into(),
            kind: SignatureKind::evm_default(),
        },
        trigger_data: wavs_types::TriggerData::default(),
    }
}

pub struct ExecAggregator;

pub struct ExecAggregatorArgs {
    pub component: String,
    pub packet: Option<String>,
    pub fuel_limit: Option<u64>,
    pub time_limit: Option<u64>,
    pub config: BTreeMap<String, String>,
}

impl ExecAggregator {
    pub async fn run(
        cli_config: &crate::config::Config,
        ExecAggregatorArgs {
            component,
            packet,
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

        // Create a minimal aggregator config from CLI config (similar to how exec component works)
        let aggregator_config = wavs_aggregator::config::Config {
            data: tempfile::tempdir()?.keep(),
            ..Default::default()
        };
        let data_dir = aggregator_config.data.clone();
        let meter = opentelemetry::global::meter("aggregator_cli");
        let metrics = utils::telemetry::AggregatorMetrics::new(meter);
        let state = wavs_aggregator::http::state::HttpState::new(aggregator_config, metrics)?;

        let wasm_bytes = read_component(&component_path)?;
        let digest = state
            .aggregator_engine
            .upload_component(wasm_bytes.clone())
            .await?;

        let env_keys = std::env::vars()
            .map(|(key, _)| key)
            .filter(|key| key.starts_with(WAVS_ENV_PREFIX))
            .collect();

        // Read packet from file or create a dummy one
        let packet = if let Some(packet_path) = packet {
            let packet_json = std::fs::read_to_string(&packet_path)?;
            serde_json::from_str(&packet_json)?
        } else {
            create_dummy_packet(digest, env_keys, config, time_limit, fuel_limit)
        };

        let mut wt_config = wasmtime::Config::new();
        wt_config.wasm_component_model(true);
        wt_config.async_support(true);
        wt_config.consume_fuel(true);
        let engine = wasmtime::Engine::new(&wt_config)?;

        let mut instance_deps = InstanceDepsBuilder {
            component: wasmtime::component::Component::new(&engine, &wasm_bytes)?,
            service: packet.service.clone(),
            workflow_id: packet.workflow_id.clone(),
            engine: &engine,
            data_dir: &data_dir,
            chain_configs: &cli_config.chains,
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
                utils::storage::db::RedbStorage::new(tempfile::tempdir()?.keep())?,
                packet.service.id().to_string(),
            ),
        }
        .build()?;

        let initial_fuel = instance_deps.store.get_fuel()?;
        let start_time = Instant::now();
        let actions =
            wavs_engine::worlds::aggregator::execute::execute_packet(&mut instance_deps, &packet)
                .await?;
        let fuel_used = initial_fuel - instance_deps.store.get_fuel()?;
        let time_elapsed = start_time.elapsed().as_millis();

        Ok(ExecAggregatorResult::Packet {
            actions: actions.into_iter().map(|a| a.into()).collect(),
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
    use std::io::Write;
    use tempfile::NamedTempFile;
    use utils::filesystem::workspace_path;
    use wavs_types::{
        AllowedHostPermission, Envelope, EnvelopeSignature, Service, ServiceManager, ServiceStatus,
        Submit, Trigger, Workflow, WorkflowId,
    };

    fn create_test_packet(component_path: &str) -> Packet {
        let wasm_bytes = read_component(component_path).unwrap();
        let digest = wavs_types::ComponentDigest::hash(&wasm_bytes);

        let service = Service {
            name: "test-service".to_string(),
            workflows: [(
                WorkflowId::default(),
                Workflow {
                    trigger: Trigger::Manual,
                    component: Component {
                        source: ComponentSource::Digest(digest),
                        permissions: Permissions {
                            allowed_http_hosts: AllowedHostPermission::All,
                            file_system: true,
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
                    },
                    submit: Submit::None,
                },
            )]
            .into(),
            status: ServiceStatus::Active,
            manager: ServiceManager::Evm {
                chain: "evm:anvil".parse().unwrap(),
                address: alloy_primitives::Address::ZERO,
            },
        };

        Packet {
            service,
            workflow_id: WorkflowId::default(),
            envelope: Envelope {
                eventId: [0u8; 20].into(),
                ordering: [0u8; 12].into(),
                payload: b"test data".to_vec().into(),
            },
            signature: EnvelopeSignature {
                data: alloy_primitives::Signature::from_bytes_and_parity(&[0u8; 64], false).into(),
                kind: SignatureKind::evm_default(),
            },
            trigger_data: wavs_types::TriggerData::default(),
        }
    }

    #[tokio::test]
    async fn test_exec_aggregator_packet() {
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("simple_aggregator.wasm")
            .to_string_lossy()
            .to_string();

        let packet = create_test_packet(&component_path);
        let mut packet_file = NamedTempFile::new().unwrap();
        packet_file
            .write_all(serde_json::to_string(&packet).unwrap().as_bytes())
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
            packet: Some(packet_file.path().to_string_lossy().to_string()),
            fuel_limit: None,
            time_limit: None,
            config,
        };

        let result = ExecAggregator::run(&crate::config::Config::default(), args)
            .await
            .unwrap();

        match result {
            ExecAggregatorResult::Packet { actions, .. } => {
                assert_eq!(actions.len(), 1);
                match &actions[0] {
                    wavs_types::AggregatorAction::Submit(submit) => {
                        assert_eq!(submit.chain, "evm:31337");
                        assert_eq!(submit.contract_address, vec![0u8; 20]);
                    }
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
            packet: None,
            fuel_limit: None,
            time_limit: None,
            config,
        };

        let result = ExecAggregator::run(&crate::config::Config::default(), args)
            .await
            .unwrap();

        match result {
            ExecAggregatorResult::Packet { actions, .. } => {
                assert_eq!(actions.len(), 1);
                match &actions[0] {
                    wavs_types::AggregatorAction::Submit(submit) => {
                        assert_eq!(submit.chain, "evm:31337");
                        assert_eq!(submit.contract_address, vec![0u8; 20]);
                    }
                    _ => panic!("Expected Submit action, got {:?}", actions[0]),
                }
            }
        }
    }
}
