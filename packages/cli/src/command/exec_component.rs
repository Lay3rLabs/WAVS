use anyhow::Result;
use std::path::PathBuf;
use wasmtime::{
    component::{Component, Linker},
    Config as WTConfig, Engine as WTEngine,
};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::util::{read_component, ComponentInput};

pub struct ExecComponent {
    pub output_bytes: Vec<u8>,
    pub gas_used: u64,
}

impl std::fmt::Display for ExecComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExecComponent")
    }
}

pub struct ExecComponentArgs {
    pub component: PathBuf,
    pub input: ComponentInput,
}

impl ExecComponent {
    pub async fn run(ExecComponentArgs { component, input }: ExecComponentArgs) -> Result<Self> {
        let wasm_bytes = read_component(component)?;
        exec_component(wasm_bytes, input.decode()?).await
    }
}

// This is pretty much all just copy/pasted from wavs... see over there for explanation :)
async fn exec_component(wasm_bytes: Vec<u8>, input_bytes: Vec<u8>) -> Result<ExecComponent> {
    let mut config = WTConfig::new();
    config.wasm_component_model(true);
    config.async_support(true);
    config.consume_fuel(true);

    let engine = WTEngine::new(&config)?;
    let app_data_dir = tempfile::tempdir()?.into_path();

    let component = Component::new(&engine, &wasm_bytes)?;

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_async(&mut linker)?;
    wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker)?;

    let mut builder = WasiCtxBuilder::new();
    builder
        .preopened_dir(&app_data_dir, ".", DirPerms::all(), FilePerms::all())
        .expect("preopen failed");

    let env: Vec<_> = std::env::vars()
        .filter(|(key, _)| key.starts_with("WAVS_ENV"))
        .collect();

    if !env.is_empty() {
        builder.envs(&env);
    }

    let ctx = builder.build();

    let host = Host {
        table: wasmtime::component::ResourceTable::new(),
        ctx,
        http: WasiHttpCtx::new(),
    };

    let mut store = wasmtime::Store::new(&engine, host);
    store.set_fuel(u64::MAX)?;

    let instance = LayerTriggerWorld::instantiate_async(&mut store, &component, &linker)
        .await
        .expect("Wasm instantiate failed");

    let input = TriggerAction {
        config: lay3r::avs::layer_types::TriggerConfig {
            service_id: "service-1".to_string(),
            workflow_id: "default".to_string(),
            trigger_source: lay3r::avs::layer_types::TriggerSource::Manual,
        },
        data: lay3r::avs::layer_types::TriggerData::Raw(input_bytes),
    };

    let response = instance
        .call_run(&mut store, &input.try_into()?)
        .await?
        .map_err(|e| anyhow::anyhow!("Wasm call failed: {:?}", e))?;

    let gas_used = u64::MAX - store.get_fuel()?;

    Ok(ExecComponent {
        output_bytes: response,
        gas_used,
    })
}

struct Host {
    pub(crate) table: wasmtime::component::ResourceTable,
    pub(crate) ctx: WasiCtx,
    pub(crate) http: WasiHttpCtx,
}

impl WasiView for Host {
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

impl WasiHttpView for Host {
    fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }
}
// https://docs.rs/wasmtime/latest/wasmtime/component/macro.bindgen.html#options-reference

use wasmtime::component::bindgen;

bindgen!({
    world: "layer-trigger-world",
    path: "../../sdk/wit",
    async: {
        only_imports: []
    }
});
