use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;
use utils::{config::ConfigBuilder, context::AppContext};
use wavs_aggregator::{config::Config as AggregatorConfig, init_tracing_tests, run_server};
use wavs_types::{
    Component, ComponentDigest, ComponentSource, Envelope, EnvelopeSignature, Packet, Service,
    ServiceManager, ServiceStatus, Submit, Trigger, Workflow, WorkflowID,
};

use crate::{args::AggregatorEntryPoint, config::Config, util::ComponentInput};

pub struct ExecAggregator;

pub struct ExecAggregatorArgs {
    pub entry_point: AggregatorEntryPoint,
    pub aggregator_config: Option<PathBuf>,
    pub component: Option<String>,
    pub input: Option<String>,
    pub service_id: Option<String>,
    pub workflow_id: Option<String>,
}

impl ExecAggregator {
    pub async fn run(
        cli_config: &Config,
        args: ExecAggregatorArgs,
    ) -> Result<ExecAggregatorResult> {
        match args.entry_point {
            AggregatorEntryPoint::Server => {
                Self::run_server(cli_config, args.aggregator_config).await
            }
            AggregatorEntryPoint::ExecutePacket => Self::execute_packet(cli_config, args).await,
            AggregatorEntryPoint::ExecuteTimer => Self::execute_timer(cli_config, args).await,
            AggregatorEntryPoint::ExecuteSubmit => Self::execute_submit(cli_config, args).await,
        }
    }

    async fn run_server(
        _cli_config: &Config,
        aggregator_config_path: Option<PathBuf>,
    ) -> Result<ExecAggregatorResult> {
        tracing::info!("Starting aggregator server...");

        let aggregator_config = Self::load_aggregator_config(aggregator_config_path)?;
        let ctx = AppContext::new();

        init_tracing_tests();

        let handle = std::thread::spawn(move || {
            run_server(ctx, aggregator_config);
        });

        handle
            .join()
            .map_err(|e| anyhow::anyhow!("Server thread panicked: {:?}", e))?;

        Ok(ExecAggregatorResult::Server)
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
        let service_id = args
            .service_id
            .context("Service ID is required for execute-packet")?;
        let workflow_id = args
            .workflow_id
            .context("Workflow ID is required for execute-packet")?;

        tracing::info!("Executing packet with component: {}", component_path);

        let aggregator_config = Self::load_aggregator_config(args.aggregator_config)?;
        let state = wavs_aggregator::http::state::HttpState::new(aggregator_config)?;

        let component = Self::load_component(&component_path)?;
        let packet = Self::create_packet(input, service_id, workflow_id)?;

        let actions = state
            .aggregator_engine
            .execute_packet(&component, &packet)
            .await?;

        Ok(ExecAggregatorResult::Packet { actions })
    }

    async fn execute_timer(
        cli_config: &Config,
        args: ExecAggregatorArgs,
    ) -> Result<ExecAggregatorResult> {
        let component_path = args
            .component
            .context("Component path is required for execute-timer")?;
        let service_id = args
            .service_id
            .context("Service ID is required for execute-timer")?;
        let workflow_id = args
            .workflow_id
            .context("Workflow ID is required for execute-timer")?;

        tracing::info!(
            "Executing timer callback with component: {}",
            component_path
        );

        let aggregator_config = Self::load_aggregator_config(args.aggregator_config)?;
        let state = wavs_aggregator::http::state::HttpState::new(aggregator_config)?;

        let component = Self::load_component(&component_path)?;
        let packet = Self::create_packet(args.input.unwrap_or_default(), service_id, workflow_id)?;

        state
            .aggregator_engine
            .execute_timer_callback(&component, &packet)
            .await?;

        Ok(ExecAggregatorResult::Timer)
    }

    async fn execute_submit(
        cli_config: &Config,
        args: ExecAggregatorArgs,
    ) -> Result<ExecAggregatorResult> {
        let component_path = args
            .component
            .context("Component path is required for execute-submit")?;
        let service_id = args
            .service_id
            .context("Service ID is required for execute-submit")?;
        let workflow_id = args
            .workflow_id
            .context("Workflow ID is required for execute-submit")?;

        tracing::info!(
            "Executing submit callback with component: {}",
            component_path
        );

        let aggregator_config = Self::load_aggregator_config(args.aggregator_config)?;
        let state = wavs_aggregator::http::state::HttpState::new(aggregator_config)?;

        let component = Self::load_component(&component_path)?;
        let packet = Self::create_packet(args.input.unwrap_or_default(), service_id, workflow_id)?;
        // Create a dummy transaction receipt as Result<AnyTxHash, String>
        let tx_receipt: Result<
            wavs_engine::bindings::aggregator::world::wavs::types::chain::AnyTxHash,
            String,
        > = Err("No transaction receipt for test".to_string());

        state
            .aggregator_engine
            .execute_submit_callback(&component, &packet, tx_receipt)
            .await?;

        Ok(ExecAggregatorResult::Submit)
    }

    fn load_aggregator_config(path: Option<PathBuf>) -> Result<AggregatorConfig> {
        if let Some(path) = path {
            let args = wavs_aggregator::args::CliArgs {
                home: Some(path.parent().unwrap().to_path_buf()),
                ..Default::default()
            };
            ConfigBuilder::new(args).build()
        } else {
            Ok(AggregatorConfig::default())
        }
    }

    fn load_component(path: &str) -> Result<Component> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("Failed to read component from {}", path))?;
        let digest = ComponentDigest::hash(&bytes);
        Ok(Component::new(ComponentSource::Digest(digest)))
    }

    fn create_packet(
        data: String,
        _service_id_str: String,
        workflow_id_str: String,
    ) -> Result<Packet> {
        let workflow_id = WorkflowID::new(workflow_id_str)?;

        let input = ComponentInput::new(data);
        let packet_bytes = input.decode()?;

        // Create a dummy workflow for the packet
        let workflow = Workflow {
            trigger: Trigger::Manual,
            component: Component::new(ComponentSource::Digest(ComponentDigest::hash(b"test"))),
            submit: Submit::None,
        };

        let mut workflows = BTreeMap::new();
        workflows.insert(workflow_id.clone(), workflow);

        Ok(Packet {
            service: Service {
                name: "ExecAggregatorService".to_string(),
                status: ServiceStatus::Active,
                manager: ServiceManager::Evm {
                    chain_name: "31337".try_into().unwrap(),
                    address: alloy_primitives::Address::ZERO,
                },
                workflows,
            },
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
    Server,
    Packet {
        actions: Vec<wavs_aggregator::engine::AggregatorAction>,
    },
    Timer,
    Submit,
}

impl std::fmt::Display for ExecAggregatorResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecAggregatorResult::Server => write!(f, "Aggregator server started"),
            ExecAggregatorResult::Packet { actions } => {
                writeln!(f, "Packet execution completed")?;
                writeln!(f, "Actions generated: {}", actions.len())?;
                for (i, action) in actions.iter().enumerate() {
                    writeln!(f, "  Action {}: {:?}", i + 1, action)?;
                }
                Ok(())
            }
            ExecAggregatorResult::Timer => write!(f, "Timer callback executed successfully"),
            ExecAggregatorResult::Submit => write!(f, "Submit callback executed successfully"),
        }
    }
}
