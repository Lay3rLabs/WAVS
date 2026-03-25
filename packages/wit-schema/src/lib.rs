pub mod cache;
pub mod convert;
pub mod docs;
pub mod traverse;
pub mod types;

pub use types::SchemaOptions;

/// Generate a JSON Schema describing the exported functions of a WASM component.
///
/// This is the primary public API. It introspects the component's type information
/// (without instantiating it) and produces a JSON Schema document with the structure:
/// ```json
/// {
///   "world": "<component-world-name>",
///   "exports": {
///     "func-name": {
///       "inputSchema": { ... },
///       "outputSchema": { ... }
///     }
///   },
///   "$defs": { ... }
/// }
/// ```
pub fn generate_schema(
    engine: &wasmtime::Engine,
    component: &wasmtime::component::Component,
    _options: &SchemaOptions,
) -> anyhow::Result<serde_json::Value> {
    let _ = (engine, component);
    todo!("implement generate_schema")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_engine() -> wasmtime::Engine {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true);
        wasmtime::Engine::new(&config).expect("failed to create engine")
    }

    fn load_component(engine: &wasmtime::Engine, name: &str) -> wasmtime::component::Component {
        let path = format!(
            "{}/examples/build/components/{}.wasm",
            env!("CARGO_MANIFEST_DIR").replace("/packages/wit-schema", ""),
            name
        );
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
        wasmtime::component::Component::new(engine, &bytes)
            .unwrap_or_else(|e| panic!("failed to load component {}: {}", name, e))
    }

    #[test]
    fn test_echo_data_schema_has_exports_with_run() {
        let engine = make_engine();
        let component = load_component(&engine, "echo_data");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        assert!(schema.get("exports").is_some(), "schema must have 'exports' key");
        let exports = schema.get("exports").unwrap();
        // echo_data exports a "run" function (possibly namespaced under an instance)
        let has_run = exports.as_object().unwrap().keys().any(|k| k.contains("run"));
        assert!(has_run, "exports must contain 'run' function, got: {:?}", exports);

        let run_export = exports.as_object().unwrap().iter()
            .find(|(k, _)| k.contains("run"))
            .map(|(_, v)| v)
            .unwrap();
        assert!(run_export.get("inputSchema").is_some(), "run must have inputSchema");
        assert!(run_export.get("outputSchema").is_some(), "run must have outputSchema");
    }

    #[test]
    fn test_top_level_structure_d04() {
        let engine = make_engine();
        let component = load_component(&engine, "echo_data");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        assert!(schema.get("world").is_some(), "schema must have 'world' key");
        assert!(schema.get("exports").is_some(), "schema must have 'exports' key");
        assert!(schema.get("$defs").is_some(), "schema must have '$defs' key");
    }

    #[test]
    fn test_aggregator_multiple_exports() {
        let engine = make_engine();
        // Try timer_aggregator first, fall back to simple_aggregator
        let component = load_component(&engine, "timer_aggregator");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        let exports = schema.get("exports").unwrap().as_object().unwrap();
        // Aggregator world has 3 exports: process-input, handle-timer-callback, handle-submit-callback
        assert!(
            exports.len() >= 3,
            "aggregator should have at least 3 exports, got {}: {:?}",
            exports.len(),
            exports.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_square_simple_types() {
        let engine = make_engine();
        let component = load_component(&engine, "square");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        assert!(schema.get("exports").is_some(), "schema must have exports");
    }

    #[test]
    fn test_exports_only_d05() {
        // Verify that the schema only contains exported functions, not imports
        let engine = make_engine();
        let component = load_component(&engine, "echo_data");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        let exports = schema.get("exports").unwrap().as_object().unwrap();
        // Should not contain imported host functions like get-evm-chain-config, config-var, etc.
        for (name, _) in exports {
            assert!(
                !name.contains("get-evm-chain-config")
                    && !name.contains("config-var")
                    && !name.contains("wasi:"),
                "found imported function in exports: {}",
                name
            );
        }
    }

    #[test]
    fn test_defs_deduplication_d06() {
        let engine = make_engine();
        let component = load_component(&engine, "timer_aggregator");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        let defs = schema.get("$defs").unwrap().as_object().unwrap();
        // The aggregator has shared types across its 3 exports (e.g. aggregator-input)
        // At least some types should be deduplicated into $defs
        assert!(
            !defs.is_empty(),
            "aggregator schema should have shared types in $defs"
        );

        // Verify $ref pointers exist somewhere in the exports
        let exports_str = serde_json::to_string(schema.get("exports").unwrap()).unwrap();
        assert!(
            exports_str.contains("$ref"),
            "exports should contain $ref pointers to $defs"
        );
    }
}
