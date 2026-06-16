use std::path::PathBuf;

use anyhow::{Context, Result};
use wasmtime::{component::Component, Config as WTConfig, Engine as WTEngine};
use wit_schema::{generate_schema, SchemaOptions};

use crate::util::read_component;

pub struct WitSchemaArgs {
    pub component_path: String,
    pub wit_path: Option<PathBuf>,
}

pub fn run(args: WitSchemaArgs) -> Result<serde_json::Value> {
    let wasm_bytes = read_component(&args.component_path).context(format!(
        "Failed to read WASM component from path: {}",
        args.component_path
    ))?;

    let mut config = WTConfig::new();
    config.wasm_component_model(true);
    let engine = WTEngine::new(&config)
        .map_err(|e| anyhow::anyhow!("Failed to create Wasmtime engine: {e}"))?;

    let component = Component::new(&engine, &wasm_bytes).map_err(|e| {
        anyhow::anyhow!(
            "Failed to load WASM component. Is this a valid component (not a core module)? {e}"
        )
    })?;

    let options = SchemaOptions {
        wit_path: args.wit_path,
    };

    generate_schema(&engine, &component, &options)
        .context("Failed to generate schema from component")
}
