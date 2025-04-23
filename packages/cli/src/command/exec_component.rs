use anyhow::Result;
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
}

impl std::fmt::Display for ExecComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fuel used: \n{}", self.fuel_used)?;

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
}

impl ExecComponent {
    pub async fn run(
        cli_config: &Config,
        ExecComponentArgs {
            component_path,
            input,
            fuel_limit,
        }: ExecComponentArgs,
    ) -> Result<Self> {
        let wasm_bytes = read_component(&component_path)?;

        let mut config = WTConfig::new();
        config.wasm_component_model(true);
        config.async_support(true);
        config.consume_fuel(true);

        let engine = WTEngine::new(&config)?;

        let trigger_action = TriggerAction {
            config: TriggerConfig {
                service_id: ServiceID::new("service-1")?,
                workflow_id: WorkflowID::default(),
                trigger: Trigger::Manual,
            },
            data: TriggerData::Raw(input.decode()?),
        };

        let mut workflow = Workflow {
            trigger: trigger_action.config.trigger.clone(),
            component: wavs_types::Component::new(ComponentSource::Digest(Digest::new(
                &wasm_bytes,
            ))),
            submit: Submit::None,
            aggregators: Vec::new(),
        };

        workflow.component.permissions = Permissions {
            allowed_http_hosts: AllowedHostPermission::All,
            file_system: true,
        };

        workflow.component.fuel_limit = fuel_limit;

        let mut instance_deps = InstanceDepsBuilder {
            workflow,
            service_id: trigger_action.config.service_id.clone(),
            workflow_id: trigger_action.config.workflow_id.clone(),
            component: WasmtimeComponent::new(&engine, &wasm_bytes)?,
            engine: &engine,
            data_dir: tempfile::tempdir()?.into_path(),
            chain_configs: &cli_config.chains,
            log: log_wasi,
        }
        .build()?;

        let initial_fuel = instance_deps.store.get_fuel()?;
        let wasm_response = wavs_engine::execute(&mut instance_deps, trigger_action).await?;

        let fuel_used = initial_fuel - instance_deps.store.get_fuel()?;

        Ok(ExecComponent {
            wasm_response,
            fuel_used,
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
            .join("echo_raw.wasm")
            .to_string_lossy()
            .to_string();

        // First try regular utf8 string
        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new("hello world".to_string()),
            fuel_limit: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.wasm_response.unwrap().payload, b"hello world");
        assert!(result.fuel_used > 0);

        // Same idea but hex-encoded with prefix
        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new("0x68656C6C6F20776F726C64".to_string()),
            fuel_limit: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.wasm_response.unwrap().payload, b"hello world");
        assert!(result.fuel_used > 0);

        // Do not hex-decode without the prefix
        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new("68656C6C6F20776F726C64".to_string()),
            fuel_limit: None,
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
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.wasm_response.unwrap().payload, b"hello world");
        assert!(result.fuel_used > 0);
    }
}
