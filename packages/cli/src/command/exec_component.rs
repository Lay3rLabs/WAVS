use anyhow::Result;
use wasmtime::{component::Component, Config as WTConfig, Engine as WTEngine};
use wavs_engine::{bindings::world::host::LogLevel, InstanceDepsBuilder};
use wavs_types::{
    AllowedHostPermission, Digest, Permissions, ServiceConfig, ServiceID, Trigger, TriggerAction,
    TriggerConfig, TriggerData, WorkflowID,
};

use crate::{
    config::Config,
    util::{read_component, ComponentInput},
};

pub struct ExecComponent {
    pub output_bytes: Option<Vec<u8>>,
    pub fuel_used: u64,
}

impl std::fmt::Display for ExecComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fuel used: \n{}", self.fuel_used)?;

        match &self.output_bytes {
            Some(bytes) => {
                write!(
                    f,
                    "\n\nResult (hex encoded): \n{}",
                    const_hex::encode(bytes)
                )?;

                if let Ok(s) = std::str::from_utf8(bytes) {
                    write!(f, "\n\nResult (utf8): \n{}", s)?;
                }
            } 
            None => write!(f, "\n\nResult: None")?,
        }

        Ok(())
    }
}

pub struct ExecComponentArgs {
    pub component_path: String,
    pub input: ComponentInput,
    pub service_config: Option<ServiceConfig>,
    pub fuel_limit: Option<u64>,
}

impl ExecComponent {
    pub async fn run(
        cli_config: &Config,
        ExecComponentArgs {
            component_path,
            input,
            service_config,
            fuel_limit,
        }: ExecComponentArgs,
    ) -> Result<Self> {
        let wasm_bytes = read_component(&component_path)?;

        let mut config = WTConfig::new();
        config.wasm_component_model(true);
        config.async_support(true);
        config.consume_fuel(true);

        let engine = WTEngine::new(&config)?;

        let service_config = service_config.unwrap_or_default();

        let trigger = TriggerAction {
            config: TriggerConfig {
                service_id: ServiceID::new("service-1")?,
                workflow_id: WorkflowID::default(),
                trigger: Trigger::Manual,
            },
            data: TriggerData::Raw(input.decode()?),
        };

        let mut instance_deps = InstanceDepsBuilder {
            service_id: trigger.config.service_id.clone(),
            workflow_id: trigger.config.workflow_id.clone(),
            digest: Digest::new(&wasm_bytes),
            component: Component::new(&engine, &wasm_bytes)?,
            engine: &engine,
            permissions: &Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            },
            data_dir: tempfile::tempdir()?.into_path(),
            service_config: &service_config,
            chain_configs: &cli_config.chains,
            log: log_wasi,
            fuel_limit,
        }
        .build()?;

        let initial_fuel = instance_deps.store.get_fuel()?;
        let response = wavs_engine::execute(&mut instance_deps, trigger).await?;

        let fuel_used = initial_fuel - instance_deps.store.get_fuel()?;

        Ok(ExecComponent {
            output_bytes: response,
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
            service_config: None,
            fuel_limit: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.output_bytes.unwrap(), b"hello world");
        assert!(result.fuel_used > 0);

        // Same idea but hex-encoded with prefix
        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new("0x68656C6C6F20776F726C64".to_string()),
            service_config: None,
            fuel_limit: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.output_bytes.unwrap(), b"hello world");
        assert!(result.fuel_used > 0);

        // Do not hex-decode without the prefix
        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new("68656C6C6F20776F726C64".to_string()),
            service_config: None,
            fuel_limit: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.output_bytes.unwrap(), b"68656C6C6F20776F726C64");
        assert!(result.fuel_used > 0);

        // And filepath

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"hello world").unwrap();

        let args = ExecComponentArgs {
            component_path: component_path.clone(),
            input: ComponentInput::new(format!("@{}", file.path().to_string_lossy())),
            service_config: None,
            fuel_limit: None,
        };

        let result = ExecComponent::run(&Config::default(), args).await.unwrap();

        assert_eq!(result.output_bytes.unwrap(), b"hello world");
        assert!(result.fuel_used > 0);
    }
}
