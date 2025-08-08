use anyhow::Result;
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
    pub aggregator_config: wavs_aggregator::config::Config,
    pub component: String,
    pub packet: Option<String>,
    pub fuel_limit: Option<u64>,
    pub time_limit: Option<u64>,
    pub chain_name: String,
    pub service_handler: String,
}

impl ExecAggregator {
    pub async fn run(args: ExecAggregatorArgs) -> Result<ExecAggregatorResult> {
        let component_path = args.component;

        tracing::info!(
            "Executing packet with aggregator component: {}",
            component_path
        );

        let mut aggregator_config = args.aggregator_config;
        aggregator_config.data = tempfile::tempdir()?.keep();
        let state = wavs_aggregator::http::state::HttpState::new(aggregator_config)?;

        let wasm_bytes = read_component(&component_path)?;
        let digest = state.aggregator_engine.upload_component(wasm_bytes).await?;
        let component = Component {
            source: ComponentSource::Digest(digest.clone()),
            permissions: Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            },
            fuel_limit: args.fuel_limit,
            time_limit_seconds: args.time_limit,
            config: [
                ("chain_name".to_string(), args.chain_name.clone()),
                ("service_handler".to_string(), args.service_handler.clone()),
            ]
            .into_iter()
            .collect(),
            env_keys: Default::default(),
        };

        // Read packet from file or create a dummy one
        let packet = if let Some(packet_path) = args.packet {
            let packet_json = std::fs::read_to_string(&packet_path)?;
            serde_json::from_str(&packet_json)?
        } else {
            create_dummy_packet(digest)
        };

        let actions = state
            .aggregator_engine
            .execute_packet(&component, &packet)
            .await?;

        Ok(ExecAggregatorResult::Packet { actions })
    }
}

pub enum ExecAggregatorResult {
    Packet {
        actions: Vec<wavs_aggregator::engine::AggregatorAction>,
    },
}

impl std::fmt::Display for ExecAggregatorResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecAggregatorResult::Packet { actions } => {
                writeln!(f, "Packet execution completed")?;
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

        let args = ExecAggregatorArgs {
            aggregator_config: wavs_aggregator::config::Config::default(),
            component: component_path,
            packet: Some(packet_file.path().to_string_lossy().to_string()),
            fuel_limit: None,
            time_limit: None,
            chain_name: "31337".to_string(),
            service_handler: "0x0000000000000000000000000000000000000000".to_string(),
        };

        let result = ExecAggregator::run(args).await.unwrap();

        match result {
            ExecAggregatorResult::Packet { actions } => {
                assert_eq!(actions.len(), 1);
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

        let args = ExecAggregatorArgs {
            aggregator_config: wavs_aggregator::config::Config::default(),
            component: component_path,
            packet: None,
            fuel_limit: None,
            time_limit: None,
            chain_name: "31337".to_string(),
            service_handler: "0x0000000000000000000000000000000000000000".to_string(),
        };

        let result = ExecAggregator::run(args).await.unwrap();

        match result {
            ExecAggregatorResult::Packet { actions } => {
                assert_eq!(actions.len(), 1);
            }
        }
    }
}
