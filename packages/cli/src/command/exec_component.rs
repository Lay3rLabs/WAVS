use std::{collections::BTreeMap, time::Instant};

use anyhow::{Context, Result};
use utils::config::WAVS_ENV_PREFIX;
use wasmtime::{component::Component as WasmtimeComponent, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{bindings::world::host::LogLevel, InstanceDepsBuilder};
use wavs_types::{
    AllowedHostPermission, ComponentSource, Digest, Permissions, ServiceID, Submit, Trigger,
    TriggerAction, TriggerConfig, TriggerData, WasmResponse, Workflow, WorkflowID,
};

use crate::{
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

        let trigger_action = TriggerAction {
            config: TriggerConfig {
                service_id: ServiceID::new("service-1")?,
                workflow_id: WorkflowID::default(),
                trigger: Trigger::Manual,
            },
            data: TriggerData::Raw(input.decode().context(format!(
                "Failed to decode input '{}' for component execution",
                input.into_string()
            ))?),
        };

        // Automatically pick up all env vars with the WAVS_ENV_PREFIX
        let env_keys = std::env::vars()
            .map(|(key, _)| key)
            .filter(|key| key.starts_with(WAVS_ENV_PREFIX))
            .collect();

        let workflow = Workflow {
            trigger: trigger_action.config.trigger.clone(),
            component: wavs_types::Component {
                source: ComponentSource::Digest(Digest::new(&wasm_bytes)),
                permissions: Permissions {
                    allowed_http_hosts: AllowedHostPermission::All,
                    file_system: true,
                },
                fuel_limit,
                time_limit_seconds: time_limit,
                config,
                env_keys,
            },
            submit: Submit::None,
            aggregators: Vec::new(),
        };

        let service = wavs_types::Service {
            id: trigger_action.config.service_id.clone(),
            name: "Exec Service".to_string(),
            workflows: BTreeMap::from([(trigger_action.config.workflow_id.clone(), workflow)]),
            status: wavs_types::ServiceStatus::Active,
            manager: wavs_types::ServiceManager::Evm { 
                chain_name: "exec".parse().unwrap(), 
                address: Default::default()
            }
        };

        let mut instance_deps = InstanceDepsBuilder {
            service,
            workflow_id: trigger_action.config.workflow_id.clone(),
            component: WasmtimeComponent::new(&engine, &wasm_bytes)?,
            engine: &engine,
            data_dir: tempfile::tempdir()?.keep(),
            chain_configs: &cli_config.chains,
            log: log_wasi,
            max_execution_seconds: Some(u64::MAX),
            max_wasm_fuel: Some(u64::MAX),
        }
        .build()
        .context("Failed to build instance dependencies for component execution")?;

        let initial_fuel = instance_deps
            .store
            .get_fuel()
            .context("Failed to get initial fuel value from the instance store")?;
        let start_time = Instant::now();
        let wasm_response = match wavs_engine::execute(&mut instance_deps, trigger_action).await {
            Ok(response) => response,
            Err(e) => {
                eprintln!("Error executing component: {}", e);
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
    service_id: &ServiceID,
    workflow_id: &WorkflowID,
    digest: &Digest,
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
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.wasm_response.unwrap().payload, b"env-value");
        assert!(result.fuel_used > 0);
    }
}
