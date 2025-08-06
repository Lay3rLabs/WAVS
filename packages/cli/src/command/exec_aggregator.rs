use anyhow::{Context, Result};
use utils::service::fetch_service;
use wavs_types::{
    Component, ComponentSource, Envelope, EnvelopeSignature, Packet, Service, ServiceManager,
    ServiceStatus, Submit, Trigger, Workflow, WorkflowID,
};

use crate::{
    args::AggregatorEntryPoint,
    config::Config,
    util::{read_component, ComponentInput},
};

pub struct ExecAggregator;

pub struct ExecAggregatorArgs {
    pub entry_point: AggregatorEntryPoint,
    pub aggregator_config: wavs_aggregator::config::Config,
    pub component: Option<String>,
    pub input: Option<String>,
    pub service_id: Option<String>,
    pub workflow_id: Option<String>,
    pub service_url: Option<String>,
    pub chain_name: String,
    pub service_handler: String,
}

impl ExecAggregator {
    pub async fn run(
        cli_config: &Config,
        args: ExecAggregatorArgs,
    ) -> Result<ExecAggregatorResult> {
        match args.entry_point {
            AggregatorEntryPoint::ExecutePacket => Self::execute_packet(cli_config, args).await,
        }
    }

    async fn execute_packet(
        cli_config: &Config,
        args: ExecAggregatorArgs,
    ) -> Result<ExecAggregatorResult> {
        let component_path = args
            .component
            .context("Component path is required for execute-packet")?;
        let input = args
            .input
            .context("Input data is required for execute-packet")?;
        let workflow_id = args
            .workflow_id
            .context("Workflow ID is required for execute-packet")?;

        tracing::info!("Executing packet with component: {}", component_path);

        let mut aggregator_config = args.aggregator_config;
        aggregator_config.data = tempfile::tempdir()?.keep();
        let state = wavs_aggregator::http::state::HttpState::new(aggregator_config)?;

        let wasm_bytes = read_component(&component_path)?;
        let digest = state.aggregator_engine.upload_component(wasm_bytes).await?;
        let component = Component {
            source: ComponentSource::Digest(digest),
            permissions: wavs_types::Permissions::default(),
            fuel_limit: Some(u64::MAX),
            time_limit_seconds: Some(10),
            config: [
                ("chain_name".to_string(), args.chain_name.clone()),
                ("service_handler".to_string(), args.service_handler.clone()),
            ]
            .into_iter()
            .collect(),
            env_keys: Default::default(),
        };

        let service = if let Some(service_url) = args.service_url {
            fetch_service(&service_url, &cli_config.ipfs_gateway).await?
        } else {
            Service {
                name: args
                    .service_id
                    .unwrap_or_else(|| "test-aggregator-service".to_string()),
                workflows: [(
                    WorkflowID::new(workflow_id.clone())?,
                    Workflow {
                        trigger: Trigger::Manual,
                        component: component.clone(),
                        submit: Submit::None,
                    },
                )]
                .into(),
                status: ServiceStatus::Active,
                manager: ServiceManager::Evm {
                    chain_name: "evm".parse()?,
                    address: alloy_primitives::Address::ZERO,
                },
            }
        };

        let packet = Self::create_packet(input, service, workflow_id)?;

        let actions = state
            .aggregator_engine
            .execute_packet(&component, &packet)
            .await?;

        Ok(ExecAggregatorResult::Packet { actions })
    }

    fn create_packet(data: String, service: Service, workflow_id_str: String) -> Result<Packet> {
        let workflow_id = WorkflowID::new(workflow_id_str)?;

        let input = ComponentInput::new(data);
        let packet_bytes = input.decode()?;

        Ok(Packet {
            service,
            workflow_id,
            envelope: Envelope {
                eventId: [0u8; 20].into(),
                ordering: [0u8; 12].into(),
                payload: packet_bytes.into(),
            },
            signature: EnvelopeSignature::Secp256k1(
                alloy_primitives::Signature::from_bytes_and_parity(&[0u8; 64], false),
            ),
        })
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

    #[tokio::test]
    async fn test_exec_aggregator_packet() {
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("simple_aggregator.wasm")
            .to_string_lossy()
            .to_string();

        let args = ExecAggregatorArgs {
            entry_point: AggregatorEntryPoint::ExecutePacket,
            aggregator_config: wavs_aggregator::config::Config::default(),
            component: Some(component_path),
            input: Some("test data".to_string()),
            service_id: Some("test-service".to_string()),
            workflow_id: Some("test-workflow".to_string()),
            service_url: None,
            chain_name: "31337".to_string(),
            service_handler: "0x0000000000000000000000000000000000000000".to_string(),
        };

        let result = ExecAggregator::run(&Config::default(), args).await.unwrap();

        match result {
            ExecAggregatorResult::Packet { actions } => {
                assert_eq!(actions.len(), 1);
            }
        }
    }

    #[tokio::test]
    async fn test_exec_aggregator_with_hex_input() {
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("simple_aggregator.wasm")
            .to_string_lossy()
            .to_string();

        let args = ExecAggregatorArgs {
            entry_point: AggregatorEntryPoint::ExecutePacket,
            aggregator_config: wavs_aggregator::config::Config::default(),
            component: Some(component_path),
            input: Some("0x68656C6C6F".to_string()), // "hello" in hex
            service_id: Some("test-service".to_string()),
            workflow_id: Some("test-workflow".to_string()),
            service_url: None,
            chain_name: "31337".to_string(),
            service_handler: "0x0000000000000000000000000000000000000000".to_string(),
        };

        let result = ExecAggregator::run(&Config::default(), args).await.unwrap();

        match result {
            ExecAggregatorResult::Packet { .. } => {}
        }
    }

    #[tokio::test]
    async fn test_exec_aggregator_with_file_input() {
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("simple_aggregator.wasm")
            .to_string_lossy()
            .to_string();

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"file content").unwrap();

        let args = ExecAggregatorArgs {
            entry_point: AggregatorEntryPoint::ExecutePacket,
            aggregator_config: wavs_aggregator::config::Config::default(),
            component: Some(component_path),
            input: Some(format!("@{}", file.path().to_string_lossy())),
            service_id: Some("test-service".to_string()),
            workflow_id: Some("test-workflow".to_string()),
            service_url: None,
            chain_name: "31337".to_string(),
            service_handler: "0x0000000000000000000000000000000000000000".to_string(),
        };

        let result = ExecAggregator::run(&Config::default(), args).await.unwrap();

        match result {
            ExecAggregatorResult::Packet { .. } => {}
        }
    }
}
