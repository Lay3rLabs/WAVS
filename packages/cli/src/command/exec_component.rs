use anyhow::Result;
use wasmtime::{component::Component, Config as WTConfig, Engine as WTEngine};
use wavs_engine::InstanceDepsBuilder;
use wavs_types::{
    AllowedHostPermission, Permissions, ServiceConfig, ServiceID, Trigger, TriggerAction,
    TriggerConfig, TriggerData, WorkflowID,
};

use crate::{
    config::Config,
    util::{read_component, ComponentInput},
};

pub struct ExecComponent {
    pub output_bytes: Vec<u8>,
    pub gas_used: u64,
}

impl std::fmt::Display for ExecComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gas used: \n{}", self.gas_used)?;

        write!(
            f,
            "\n\nResult (hex encoded): \n{}",
            const_hex::encode(&self.output_bytes)
        )?;

        if let Ok(s) = std::str::from_utf8(&self.output_bytes) {
            write!(f, "\n\nResult (utf8): \n{}", s)?;
        }

        Ok(())
    }
}

pub struct ExecComponentArgs {
    pub component_path: String,
    pub input: ComponentInput,
    pub service_config: Option<ServiceConfig>,
}

impl ExecComponent {
    pub async fn run(
        cli_config: &Config,
        ExecComponentArgs {
            component_path,
            input,
            service_config,
        }: ExecComponentArgs,
    ) -> Result<Self> {
        let wasm_bytes = read_component(&component_path)?;

        let mut config = WTConfig::new();
        config.wasm_component_model(true);
        config.async_support(true);
        config.consume_fuel(true);

        let engine = WTEngine::new(&config)?;

        let mut service_config = service_config.unwrap_or_default();

        service_config.fuel_limit = u64::MAX;

        let mut instance_deps = InstanceDepsBuilder {
            component: Component::new(&engine, &wasm_bytes)?,
            engine: &engine,
            permissions: &Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            },
            data_dir: tempfile::tempdir()?.into_path(),
            service_config: &service_config,
            chain_configs: &cli_config.chains,
        }
        .build()?;

        let trigger = TriggerAction {
            config: TriggerConfig {
                service_id: ServiceID::new("service-1")?,
                workflow_id: WorkflowID::new("default")?,
                trigger: Trigger::Manual,
            },
            data: TriggerData::Raw(input.decode()?),
        };

        let response = wavs_engine::execute(&mut instance_deps, trigger).await?;

        let gas_used = u64::MAX - instance_deps.store.get_fuel()?;

        Ok(ExecComponent {
            output_bytes: response,
            gas_used,
        })
    }
}
