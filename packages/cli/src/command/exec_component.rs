use std::{
    collections::BTreeMap,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use alloy_primitives::FixedBytes;
use anyhow::{Context, Result};
use utils::{config::WAVS_ENV_PREFIX, storage::db::WavsDb};
use wasmtime::{component::Component as WasmtimeComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{
    bindings::operator::world::host::LogLevel,
    worlds::instance::{HostComponentLogger, InstanceData, InstanceDepsBuilder},
};
use wavs_types::{
    AllowedHostPermission, ChainKey, ComponentDigest, ComponentSource, Permissions, ServiceId,
    Submit, Timestamp, Trigger, TriggerAction, TriggerConfig, TriggerData, WasmResponse, Workflow,
    WorkflowId,
};

use crate::{
    args::TriggerKind,
    config::Config,
    util::{read_component, ComponentInput},
};

pub struct ExecComponent {
    pub wasm_response: Option<WasmResponse>,
    pub fuel_used: u64,
    pub time_elapsed: u128,
}

impl std::fmt::Display for ExecComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fuel used: \n{}", self.fuel_used)?;
        if self.time_elapsed > 0 {
            write!(f, "\n\nTime elapsed (ms): \n{}", self.time_elapsed)?;
        }
        match &self.wasm_response {
            Some(wasm_response) => {
                write!(
                    f,
                    "\n\nResult (hex encoded): \n{}",
                    const_hex::encode(&wasm_response.payload)
                )?;

                if let Ok(s) = std::str::from_utf8(&wasm_response.payload) {
                    write!(f, "\n\nResult (utf8): \n{}", s)?;
                }

                write!(
                    f,
                    "\n\nOrdering: \n{}",
                    wasm_response.ordering.unwrap_or_default()
                )?;
            }
            None => write!(f, "\n\nResult: None")?,
        }

        Ok(())
    }
}

pub struct ExecComponentArgs {
    pub component_path: String,
    pub input: ComponentInput,
    pub fuel_limit: Option<u64>,
    pub time_limit: Option<u64>,
    pub config: BTreeMap<String, String>,
    pub simulates_trigger: Option<TriggerKind>,
}

impl ExecComponent {
    pub async fn run(
        cli_config: &Config,
        ExecComponentArgs {
            component_path,
            input,
            fuel_limit,
            time_limit,
            config,
            simulates_trigger,
        }: ExecComponentArgs,
    ) -> Result<Self> {
        let wasm_bytes = read_component(&component_path).context(format!(
            "Failed to read WASM component from path: {}",
            component_path
        ))?;

        let mut wt_config = WTConfig::new();
        wt_config.wasm_component_model(true);
        wt_config.async_support(true);
        wt_config.consume_fuel(true);

        let engine = WTEngine::new(&wt_config)
            .context("Failed to create Wasmtime engine with the specified configuration")?;

        // Automatically pick up all env vars with the WAVS_ENV_PREFIX
        let env_keys = std::env::vars()
            .map(|(key, _)| key)
            .filter(|key| key.starts_with(WAVS_ENV_PREFIX))
            .collect();

        let workflow = Workflow {
            trigger: Trigger::Manual,
            component: wavs_types::Component {
                source: ComponentSource::Digest(ComponentDigest::hash(&wasm_bytes)),
                permissions: Permissions {
                    allowed_http_hosts: AllowedHostPermission::All,
                    file_system: true,
                    raw_sockets: true,
                    dns_resolution: true,
                },
                fuel_limit,
                time_limit_seconds: time_limit,
                config,
                env_keys,
            },
            submit: Submit::None,
        };

        let chain: ChainKey = "evm:exec".parse().unwrap();
        let service = wavs_types::Service {
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(WorkflowId::default(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm {
                chain: chain.clone(),
                address: Default::default(),
            },
        };

        let data = match simulates_trigger {
            Some(trigger_kind) => match trigger_kind {
                TriggerKind::Cron { trigger_time } => TriggerData::Cron {
                    trigger_time: Timestamp::from_nanos(trigger_time),
                },
                TriggerKind::EvmContractEvent {
                    chain,
                    contract_address,
                    log_data,
                    block_number,
                } => TriggerData::EvmContractEvent {
                    chain,
                    contract_address,
                    log_data,
                    tx_hash: FixedBytes::new(rand::random()),
                    block_number,
                    log_index: 0,
                    block_hash: FixedBytes::new(rand::random()),
                    block_timestamp: Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("Time went backwards")
                            .as_secs(),
                    ),
                    tx_index: 0,
                },
                TriggerKind::BlockInterval {
                    chain,
                    block_height,
                } => TriggerData::BlockInterval {
                    chain,
                    block_height,
                },
            },
            None => TriggerData::Raw(
                input
                    .decode()
                    .context("Failed to decode input for component execution")?,
            ),
        };

        let trigger_action = TriggerAction {
            config: TriggerConfig {
                service_id: service.id(),
                workflow_id: WorkflowId::default(),
                trigger: Trigger::Manual,
            },
            data,
        };

        let mut instance_deps = InstanceDepsBuilder {
            service,
            workflow_id: trigger_action.config.workflow_id.clone(),
            component: WasmtimeComponent::new(&engine, &wasm_bytes)?,
            data: InstanceData::new_operator(trigger_action.data.clone()),
            engine: &engine,
            data_dir: tempfile::tempdir()?.keep(),
            chain_configs: &cli_config.chains.read().unwrap(),
            log: HostComponentLogger::OperatorHostComponentLogger(log_wasi),
            keyvalue_ctx: wavs_engine::backend::wasi_keyvalue::context::KeyValueCtx::new(
                WavsDb::new().unwrap(),
                "exec_component".to_string(),
            ),
        }
        .build()
        .context("Failed to build instance dependencies for component execution")?;

        let initial_fuel = instance_deps
            .store
            .get_fuel()
            .context("Failed to get initial fuel value from the instance store")?;
        let start_time = Instant::now();
        let wasm_response = match wavs_engine::worlds::operator::execute::execute(
            &mut instance_deps,
            trigger_action,
        )
        .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!("Error executing component: {}", e);
                return Err(anyhow::anyhow!("Component execution failed: {}", e));
            }
        };

        let fuel_used = initial_fuel - instance_deps.store.get_fuel()?;

        Ok(ExecComponent {
            wasm_response,
            fuel_used,
            time_elapsed: start_time.elapsed().as_millis(),
        })
    }
}

fn log_wasi(
    service_id: &ServiceId,
    workflow_id: &WorkflowId,
    digest: &ComponentDigest,
    level: LogLevel,
    message: String,
) {
    let message = format!("[{}:{}:{}] {}", service_id, workflow_id, digest, message);

    match level {
        LogLevel::Error => tracing::error!("{}", message),
        LogLevel::Warn => tracing::warn!("{}", message),
        LogLevel::Info => tracing::info!("{}", message),
        LogLevel::Debug => tracing::debug!("{}", message),
        LogLevel::Trace => tracing::trace!("{}", message),
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use utils::filesystem::workspace_path;

    use super::*;

    #[tokio::test]
    async fn test_exec_component() {
        let component_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join("echo_data.wasm")
            .to_string_lossy()
            .to_string();

        // First try regular utf8 string
        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new("hello world".to_string()),
            fuel_limit: None,
            time_limit: None,
            config: BTreeMap::default(),
            simulates_trigger: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.wasm_response.unwrap().payload, b"hello world");
        assert!(result.fuel_used > 0);

        // Same idea but hex-encoded with prefix
        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new("0x68656C6C6F20776F726C64".to_string()),
            fuel_limit: None,
            time_limit: None,
            config: BTreeMap::default(),
            simulates_trigger: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.wasm_response.unwrap().payload, b"hello world");
        assert!(result.fuel_used > 0);

        // Do not hex-decode without the prefix
        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new("68656C6C6F20776F726C64".to_string()),
            fuel_limit: None,
            time_limit: None,
            config: BTreeMap::default(),
            simulates_trigger: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(
            result.wasm_response.unwrap().payload,
            b"68656C6C6F20776F726C64"
        );
        assert!(result.fuel_used > 0);

        // And filepath

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"hello world").unwrap();

        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new(format!("@{}", file.path().to_string_lossy())),
            fuel_limit: None,
            time_limit: None,
            config: BTreeMap::default(),
            simulates_trigger: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.wasm_response.unwrap().payload, b"hello world");
        assert!(result.fuel_used > 0);

        // Test config var usage in the Wasm component
        let mut config_map = BTreeMap::new();
        config_map.insert("my_config_key".to_string(), "config-value".to_string());

        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new("configvar:my_config_key".to_string()),
            fuel_limit: None,
            time_limit: None,
            config: config_map,
            simulates_trigger: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.wasm_response.unwrap().payload, b"config-value");
        assert!(result.fuel_used > 0);

        // Set an env var and test it via envvar:<key> lookup
        let var = format!("{}_MY_ENV_VAR", WAVS_ENV_PREFIX);
        std::env::set_var(&var, "env-value");

        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new(format!("envvar:{}", var)),
            fuel_limit: None,
            time_limit: None,
            config: BTreeMap::default(),
            simulates_trigger: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.wasm_response.unwrap().payload, b"env-value");
        assert!(result.fuel_used > 0);
    }
}
