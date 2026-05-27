//! In-process runner — executes operator and aggregator WASM components directly
//! against `wavs-engine`, without booting the full WAVS dispatcher.
//!
//! This is the **inproc** tier: the operator and aggregator run real WASM, but
//! trigger dispatch and submission orchestration are driven explicitly by the
//! test via [`lifecycle`](crate::lifecycle) helpers. Dispatcher subsystems
//! (trigger manager, submission manager, signing) are bypassed.
//!
//! Modelled after `packages/engine/tests/helpers/exec.rs`. Copied + adapted
//! rather than wrapped because that helper lives in `tests/` and has no `[lib]`
//! target.

use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use wasmtime::{component::Component as WasmtimeComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{
    backend::wasi_keyvalue::context::KeyValueCtx,
    bindings::operator::world::host::LogLevel,
    utils::error::EngineError,
    worlds::{
        aggregator::execute::{execute_input as agg_execute_input, AggregatorAction},
        instance::{HostComponentLogger, InstanceData, InstanceDepsBuilder},
        operator::execute::execute as op_execute,
    },
};

// Re-export the aggregator action types so downstream tests can match on the
// runner's output without depending on `wavs-engine` paths directly.
pub use wavs_engine::worlds::aggregator::execute::{
    AggregatorAction as RunnerAggregatorAction, SubmitAction as RunnerSubmitAction,
};
use wavs_types::{
    AggregatorInput, AllowedHostPermission, ChainConfigs, Component, ComponentDigest,
    ComponentSource, EventId, Permissions, Service, ServiceId, ServiceManager, ServiceStatus,
    SignatureKind, Submit, Trigger, TriggerAction, TriggerConfig, TriggerData, WasmResponse,
    WorkflowId,
};

use crate::service::ServiceSpec;
use utils::storage::db::WavsDb;

/// In-process runner. Holds the WASM bytes for the operator + aggregator
/// components and a synthetic [`Service`] used for engine wiring.
pub struct InProcRunner {
    component_bytes: Vec<u8>,
    aggregator_bytes: Vec<u8>,
    config: BTreeMap<String, String>,
    service: Service,
    workflow_id: WorkflowId,
    chain_configs: ChainConfigs,
    engine: WTEngine,
}

impl InProcRunner {
    /// Build a runner from a validated [`ServiceSpec`]. Reads both WASM files into
    /// memory. The synthetic [`Service`] uses [`Trigger::Manual`] — tests drive
    /// trigger emission explicitly.
    pub fn from_spec(spec: &ServiceSpec) -> Result<Self> {
        spec.validate()?;
        let component_path = spec
            .component_wasm_path()
            .ok_or_else(|| anyhow!("component_wasm not set"))?;
        let aggregator_path = spec
            .aggregator_wasm_path()
            .ok_or_else(|| anyhow!("aggregator_wasm not set"))?;
        let component_bytes = std::fs::read(component_path)
            .with_context(|| format!("read component wasm {}", component_path.display()))?;
        let aggregator_bytes = std::fs::read(aggregator_path)
            .with_context(|| format!("read aggregator wasm {}", aggregator_path.display()))?;
        let config = spec.config_var_map().clone();

        let digest = ComponentDigest::hash(&component_bytes);
        let (service, workflow_id) = synthetic_service(digest, config.clone());

        Ok(Self {
            component_bytes,
            aggregator_bytes,
            config,
            service,
            workflow_id,
            chain_configs: spec.chain_configs_ref().clone(),
            engine: build_engine()?,
        })
    }

    pub fn service(&self) -> &Service {
        &self.service
    }

    pub fn service_id(&self) -> ServiceId {
        self.service.id().clone()
    }

    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    /// Build the canonical [`TriggerAction`] for this runner from raw input
    /// bytes. The same `TriggerAction` should be reused when constructing the
    /// `AggregatorInput` for the aggregator stage so the derived `EventId`
    /// matches between operator emit and aggregator consume — production wires
    /// it the same way.
    pub fn default_trigger_action(&self, input: Vec<u8>) -> TriggerAction {
        TriggerAction {
            config: TriggerConfig {
                service_id: self.service.id().clone(),
                workflow_id: self.workflow_id.clone(),
                trigger: self
                    .service
                    .workflows
                    .values()
                    .next()
                    .unwrap()
                    .trigger
                    .clone(),
            },
            data: TriggerData::Raw(input),
        }
    }

    /// Execute the operator component once with the given raw input bytes.
    /// Returns the list of payload bytes emitted by the component.
    ///
    /// Thin wrapper around [`Self::run_component_full`] — kept for callers that
    /// only need the payload bytes. Callers feeding output into the aggregator
    /// should prefer `run_component_full` to preserve `ordering` and
    /// `event_id_salt`.
    pub async fn run_component(&self, input: Vec<u8>) -> Result<Vec<Vec<u8>>> {
        let trigger_action = self.default_trigger_action(input);
        let responses = self.run_component_full(trigger_action).await?;
        Ok(responses.into_iter().map(|r| r.payload).collect())
    }

    /// Execute the operator component and return the full set of
    /// [`WasmResponse`]s — payload bytes plus `ordering` and `event_id_salt`
    /// — so callers can build an [`AggregatorInput`] without losing fields.
    pub async fn run_component_full(
        &self,
        trigger_action: TriggerAction,
    ) -> Result<Vec<WasmResponse>> {
        let data_dir = tempfile::tempdir()?;
        let keyvalue_ctx = KeyValueCtx::new(WavsDb::new()?, "test".to_string());

        let mut deps = InstanceDepsBuilder {
            workflow_id: self.workflow_id.clone(),
            service: self.service.clone(),
            data: InstanceData::new_operator(trigger_action.data.clone()),
            component: WasmtimeComponent::new(&self.engine, &self.component_bytes)
                .map_err(|e| anyhow!("instantiate operator component: {e}"))?,
            engine: &self.engine,
            data_dir: data_dir.path().to_path_buf(),
            chain_configs: &self.chain_configs,
            log: HostComponentLogger::OperatorHostComponentLogger(log_host),
            keyvalue_ctx,
        }
        .build()
        .map_err(|e| anyhow!("build operator deps: {e}"))?;

        let responses = op_execute(
            &mut deps,
            trigger_action,
            WasmResponse::DEFAULT_MAX_PAYLOAD_SIZE,
            WasmResponse::DEFAULT_MAX_SALT_SIZE,
        )
        .await
        .map_err(map_engine_error)?;

        Ok(responses)
    }

    /// Execute the aggregator component once on a single packet input.
    /// Returns the list of [`AggregatorAction`]s the aggregator emitted.
    ///
    /// `event_id` identifies the trigger event this aggregation is for — pass a
    /// fresh id per logical event to avoid collisions with the aggregator's
    /// internal de-dup state.
    pub async fn run_aggregator(
        &self,
        event_id: EventId,
        input: AggregatorInput,
    ) -> Result<Vec<AggregatorAction>> {
        let data_dir = tempfile::tempdir()?;
        let keyvalue_ctx = KeyValueCtx::new(WavsDb::new()?, "test-agg".to_string());

        // The aggregator's synthetic service uses its own digest.
        let agg_digest = ComponentDigest::hash(&self.aggregator_bytes);
        let (agg_service, agg_wf) = synthetic_service(agg_digest, self.config.clone());

        let mut deps = InstanceDepsBuilder {
            workflow_id: agg_wf,
            service: agg_service,
            data: InstanceData::new_aggregator(event_id),
            component: WasmtimeComponent::new(&self.engine, &self.aggregator_bytes)
                .map_err(|e| anyhow!("instantiate aggregator component: {e}"))?,
            engine: &self.engine,
            data_dir: data_dir.path().to_path_buf(),
            chain_configs: &self.chain_configs,
            log: HostComponentLogger::AggregatorHostComponentLogger(log_host_agg),
            keyvalue_ctx,
        }
        .build()
        .map_err(|e| anyhow!("build aggregator deps: {e}"))?;

        agg_execute_input(&mut deps, input)
            .await
            .map_err(map_engine_error)
    }
}

fn build_engine() -> Result<WTEngine> {
    let mut cfg = WTConfig::new();
    cfg.wasm_component_model(true);
    cfg.consume_fuel(true);
    WTEngine::new(&cfg).map_err(|e| anyhow!("build wasmtime engine: {e}"))
}

fn map_engine_error(e: EngineError) -> anyhow::Error {
    match e {
        EngineError::ExecResult(s) => anyhow!("component execution failed: {s}"),
        other => anyhow!("engine error: {other}"),
    }
}

fn synthetic_service(
    digest: ComponentDigest,
    config: BTreeMap<String, String>,
) -> (Service, WorkflowId) {
    use wavs_types::Workflow;
    let workflow_id = WorkflowId::new("test-workflow").unwrap();
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
        config,
        env_keys: Default::default(),
    };
    let workflow = Workflow {
        trigger: Trigger::Manual,
        component: component.clone(),
        submit: Submit::Aggregator {
            component: Box::new(component),
            signature_kind: SignatureKind::evm_default(),
        },
    };
    let service = Service {
        name: "wavs-test-harness synthetic".to_string(),
        workflows: BTreeMap::from([(workflow_id.clone(), workflow)]),
        status: ServiceStatus::Active,
        manager: ServiceManager::Evm {
            chain: "evm:test".parse().unwrap(),
            address: Default::default(),
        },
    };
    (service, workflow_id)
}

fn log_host(
    service_id: &ServiceId,
    workflow_id: &WorkflowId,
    digest: &ComponentDigest,
    level: LogLevel,
    message: String,
) {
    let line = format!("[{service_id}:{workflow_id}:{digest}] {message}");
    match level {
        LogLevel::Error => tracing::error!(target: "wavs_test_harness::component", "{line}"),
        LogLevel::Warn => tracing::warn!(target: "wavs_test_harness::component", "{line}"),
        LogLevel::Info => tracing::info!(target: "wavs_test_harness::component", "{line}"),
        LogLevel::Debug => tracing::debug!(target: "wavs_test_harness::component", "{line}"),
        LogLevel::Trace => tracing::trace!(target: "wavs_test_harness::component", "{line}"),
    }
}

fn log_host_agg(
    service_id: &ServiceId,
    workflow_id: &WorkflowId,
    digest: &ComponentDigest,
    level: wavs_engine::bindings::aggregator::world::host::LogLevel,
    message: String,
) {
    use wavs_engine::bindings::aggregator::world::host::LogLevel as AL;
    let line = format!("[{service_id}:{workflow_id}:{digest}] {message}");
    match level {
        AL::Error => tracing::error!(target: "wavs_test_harness::aggregator", "{line}"),
        AL::Warn => tracing::warn!(target: "wavs_test_harness::aggregator", "{line}"),
        AL::Info => tracing::info!(target: "wavs_test_harness::aggregator", "{line}"),
        AL::Debug => tracing::debug!(target: "wavs_test_harness::aggregator", "{line}"),
        AL::Trace => tracing::trace!(target: "wavs_test_harness::aggregator", "{line}"),
    }
}
