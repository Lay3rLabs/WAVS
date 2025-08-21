use anyhow::Result;
use std::collections::BTreeMap;
use std::time::Instant;
use utils::config::WAVS_ENV_PREFIX;
use wavs_types::{
    AllowedHostPermission, Component, ComponentDigest, ComponentSource, Envelope,
    EnvelopeSignature, Packet, Permissions, Service, ServiceManager, ServiceStatus, Submit,
    Trigger, Workflow, WorkflowID,
};

use crate::util::read_component;

fn create_dummy_packet(digest: ComponentDigest) -> Packet {
    let service = Service {
        name: "dummy-service".to_string(),
        workflows: [(
            WorkflowID::default(),
            Workflow {
                trigger: Trigger::Manual,
                component: Component {
                    source: ComponentSource::Digest(digest),
                    permissions: Permissions::default(),
                    fuel_limit: None,
                    time_limit_seconds: None,
                    config: Default::default(),
                    env_keys: Default::default(),
                },
                submit: Submit::None,
            },
        )]
        .into(),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain_name: "dummy".parse().unwrap(),
            address: alloy_primitives::Address::ZERO,
        },
    };

    Packet {
        envelope: Envelope {
            eventId: [0u8; 20].into(),
            ordering: [0u8; 12].into(),
            payload: vec![].into(),
        },
        workflow_id: WorkflowID::default(),
        service,
        signature: EnvelopeSignature::Secp256k1(
            alloy_primitives::Signature::from_bytes_and_parity(&[0u8; 64], false),
        ),
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
        let state = wavs_aggregator::http::state::HttpState::new(aggregator_config)?;

        let wasm_bytes = read_component(&component_path)?;
        let digest = state
            .aggregator_engine
            .upload_component(wasm_bytes.clone())
            .await?;

        let env_keys = std::env::vars()
            .map(|(key, _)| key)
            .filter(|key| key.starts_with(WAVS_ENV_PREFIX))
            .collect();

        let component = Component {
            source: ComponentSource::Digest(digest.clone()),
            permissions: Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            },
            fuel_limit,
            time_limit_seconds: time_limit,
            config,
            env_keys,
        };

        // Read packet from file or create a dummy one
        let packet = if let Some(packet_path) = packet {
            let packet_json = std::fs::read_to_string(&packet_path)?;
            serde_json::from_str(&packet_json)?
        } else {
            create_dummy_packet(digest)
        };

        let mut wt_config = wasmtime::Config::new();
        wt_config.wasm_component_model(true);
        wt_config.async_support(true);
        wt_config.consume_fuel(true);
        let engine = wasmtime::Engine::new(&wt_config)?;

        let mut instance_deps = wavs_engine::worlds::aggregator::instance::AggregatorInstanceDepsBuilder {
            component: wasmtime::component::Component::new(&engine, &wasm_bytes)?,
            aggregator_component: component.clone(),
            service: packet.service.clone(),
            workflow_id: packet.workflow_id.clone(),
            engine: &engine,
            data_dir: &data_dir,
            chain_configs: &cli_config.chains,
            log: |_service_id, _workflow_id, _digest, level, message| match level {
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Error => tracing::error!("{}", message),
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Warn => tracing::warn!("{}", message),
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Info => tracing::info!("{}", message),
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Debug => tracing::debug!("{}", message),
                wavs_engine::bindings::aggregator::world::wavs::types::core::LogLevel::Trace => tracing::trace!("{}", message),
            },
            max_wasm_fuel: component.fuel_limit,
            max_execution_seconds: component.time_limit_seconds,
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
            actions,
            fuel_used,
            time_elapsed,
        })
    }
}

pub enum ExecAggregatorResult {
    Packet {
        actions: Vec<wavs_aggregator::engine::AggregatorAction>,
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
        Envelope, EnvelopeSignature, Service, ServiceManager, ServiceStatus, Submit, Trigger,
        Workflow, WorkflowID,
    };

    fn create_test_packet(component_path: &str) -> Packet {
        let wasm_bytes = read_component(component_path).unwrap();
        let digest = wavs_types::ComponentDigest::hash(&wasm_bytes);

        let service = Service {
            name: "test-service".to_string(),
            workflows: [(
                WorkflowID::default(),
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
                            ("chain_name".to_string(), "31337".to_string()),
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
                chain_name: "evm".parse().unwrap(),
                address: alloy_primitives::Address::ZERO,
            },
        };

        Packet {
            service,
            workflow_id: WorkflowID::default(),
            envelope: Envelope {
                eventId: [0u8; 20].into(),
                ordering: [0u8; 12].into(),
                payload: b"test data".to_vec().into(),
            },
            signature: EnvelopeSignature::Secp256k1(
                alloy_primitives::Signature::from_bytes_and_parity(&[0u8; 64], false),
            ),
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
            ("chain_name".to_string(), "31337".to_string()),
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
                    wavs_aggregator::engine::AggregatorAction::Submit(submit) => {
                        assert_eq!(submit.chain_name, "31337");
                        assert_eq!(submit.contract_address.raw_bytes, vec![0u8; 20]);
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
            ("chain_name".to_string(), "31337".to_string()),
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
                    wavs_aggregator::engine::AggregatorAction::Submit(submit) => {
                        assert_eq!(submit.chain_name, "31337");
                        assert_eq!(submit.contract_address.raw_bytes, vec![0u8; 20]);
                    }
                    _ => panic!("Expected Submit action, got {:?}", actions[0]),
                }
            }
        }
    }
}
