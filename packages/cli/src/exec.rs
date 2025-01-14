use wasmtime::{
    component::{Component, Linker},
    Config as WTConfig, Engine as WTEngine,
};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

// This is pretty much all just copy/pasted from wavs... see over there for explanation :)
pub struct ExecComponentResponse {
    pub output_bytes: Vec<u8>,
    pub gas_used: u64,
}

pub async fn exec_component(wasm_bytes: Vec<u8>, input_bytes: Vec<u8>) -> ExecComponentResponse {
    let mut config = WTConfig::new();
    config.wasm_component_model(true);
    config.async_support(true);
    config.consume_fuel(true);

    let engine = WTEngine::new(&config).unwrap();
    let app_data_dir = tempfile::tempdir().unwrap().into_path();

    let component = Component::new(&engine, &wasm_bytes).unwrap();

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();
    wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();

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
    store.set_fuel(u64::MAX).unwrap();

    let instance = EthTriggerWorld::instantiate_async(&mut store, &component, &linker)
        .await
        .expect("Wasm instantiate failed");

    let response = instance
        .call_process_eth_trigger(&mut store, &input_bytes)
        .await
        .unwrap()
        .unwrap();

    let gas_used = u64::MAX - store.get_fuel().unwrap();

    ExecComponentResponse {
        output_bytes: response,
        gas_used,
    }
}

pub(crate) struct Host {
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

wasmtime::component::bindgen!({
  world: "eth-trigger-world",
  path: "../../wit",
  async: true,
});
