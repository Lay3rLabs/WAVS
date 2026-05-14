pub mod cache;
pub mod convert;
pub mod docs;
pub mod traverse;
pub mod types;

pub use cache::SchemaCache;
pub use types::SchemaOptions;

use std::collections::{BTreeMap, HashMap};

use serde_json::{json, Value};
use wasmtime::component::types::Type;

/// Generate a JSON Schema describing the exported functions of a WASM component.
///
/// This is the primary public API. It introspects the component's type information
/// (without instantiating it) and produces a JSON Schema document with the structure
/// specified by D-04:
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
///
/// Only exported functions are included (D-05). Imported functions (WASI, host, etc.)
/// are excluded.
pub fn generate_schema(
    engine: &wasmtime::Engine,
    component: &wasmtime::component::Component,
    _options: &SchemaOptions,
) -> anyhow::Result<Value> {
    let component_type = component.component_type();
    let exports = traverse::gather_exports(&component_type, engine);

    let mut defs: BTreeMap<String, Value> = BTreeMap::new();
    let mut seen_types: HashMap<String, usize> = HashMap::new();
    let mut export_schemas = serde_json::Map::new();

    // First pass: generate schemas for all exports to discover shared types
    // We need two passes for proper $defs deduplication:
    // 1. First pass discovers all types and which are shared
    // 2. Second pass generates final schemas with $ref pointers

    // Collect type fingerprints across all exports to pre-populate seen_types
    for (_name, func) in &exports {
        for (_param_name, param_ty) in func.params() {
            count_type_occurrences(&param_ty, &mut seen_types);
        }
        for result_ty in func.results() {
            count_type_occurrences(&result_ty, &mut seen_types);
        }
    }

    // Reset counts but keep fingerprints that appeared more than once
    let shared_fingerprints: HashMap<String, usize> = seen_types
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(fp, _)| (fp.clone(), 0))
        .collect();
    seen_types = shared_fingerprints;

    // Second pass: generate actual schemas, using $ref for shared types
    for (name, func) in &exports {
        let input_schema = build_input_schema(func, &mut defs, &mut seen_types);
        let output_schema = build_output_schema(func, &mut defs, &mut seen_types);

        let mut entry = serde_json::Map::new();
        entry.insert("inputSchema".to_string(), input_schema);
        entry.insert("outputSchema".to_string(), output_schema);

        export_schemas.insert(name.clone(), Value::Object(entry));
    }

    // Assemble top-level schema per D-04
    let schema = json!({
        "world": "unknown",
        "exports": Value::Object(export_schemas),
        "$defs": defs
    });

    Ok(schema)
}

/// Generate schema with caching and optional doc enrichment.
///
/// Wraps `generate_schema` with:
/// 1. Digest-based cache lookup (skips regeneration for known components)
/// 2. Optional WIT source doc comment enrichment (D-07)
/// 3. Cache storage of the result
pub fn generate_schema_cached(
    engine: &wasmtime::Engine,
    component: &wasmtime::component::Component,
    wasm_bytes: &[u8],
    options: &SchemaOptions,
    cache: &SchemaCache,
) -> anyhow::Result<Value> {
    let digest = wavs_types::ComponentDigest::hash(wasm_bytes);

    // Check cache first
    if let Some(cached) = cache.get(&digest) {
        tracing::debug!("Schema cache hit for {}", digest);
        return Ok(cached);
    }

    // Generate schema
    let mut schema = generate_schema(engine, component, options)?;

    // Optionally enrich with doc comments from WIT source
    if let Some(ref wit_path) = options.wit_path {
        docs::enrich_with_docs(&mut schema, wit_path)?;
    }

    // Store in cache
    cache.put(digest, schema.clone());

    Ok(schema)
}

/// Count type occurrences for deduplication discovery (first pass).
fn count_type_occurrences(ty: &Type, seen_types: &mut HashMap<String, usize>) {
    if let Some(fingerprint) = type_fingerprint_for_counting(ty) {
        *seen_types.entry(fingerprint).or_insert(0) += 1;
    }

    // Recurse into complex types
    match ty {
        Type::Record(record) => {
            for field in record.fields() {
                count_type_occurrences(&field.ty, seen_types);
            }
        }
        Type::Variant(variant) => {
            for case in variant.cases() {
                if let Some(ref payload_ty) = case.ty {
                    count_type_occurrences(payload_ty, seen_types);
                }
            }
        }
        Type::List(list) => {
            count_type_occurrences(&list.ty(), seen_types);
        }
        Type::Option(opt) => {
            count_type_occurrences(&opt.ty(), seen_types);
        }
        Type::Result(result) => {
            if let Some(ok) = result.ok() {
                count_type_occurrences(&ok, seen_types);
            }
            if let Some(err) = result.err() {
                count_type_occurrences(&err, seen_types);
            }
        }
        Type::Tuple(tuple) => {
            for item_ty in tuple.types() {
                count_type_occurrences(&item_ty, seen_types);
            }
        }
        _ => {}
    }
}

/// Same as convert module's fingerprint but accessible here for counting.
fn type_fingerprint_for_counting(ty: &Type) -> Option<String> {
    match ty {
        Type::Record(record) => {
            let fields: Vec<String> = record.fields().map(|f| f.name.to_string()).collect();
            Some(format!("record:{}", fields.join("|")))
        }
        Type::Variant(variant) => {
            let cases: Vec<String> = variant.cases().map(|c| c.name.to_string()).collect();
            Some(format!("variant:{}", cases.join("|")))
        }
        Type::Enum(enum_ty) => {
            let names: Vec<String> = enum_ty.names().map(|n| n.to_string()).collect();
            Some(format!("enum:{}", names.join("|")))
        }
        Type::Flags(flags) => {
            let names: Vec<String> = flags.names().map(|n| n.to_string()).collect();
            Some(format!("flags:{}", names.join("|")))
        }
        _ => None,
    }
}

/// Build the inputSchema for a function.
fn build_input_schema(
    func: &wasmtime::component::types::ComponentFunc,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    let params: Vec<_> = func.params().collect();

    match params.len() {
        0 => json!({"type": "object", "properties": {}, "additionalProperties": false}),
        1 => {
            let (name, ty) = &params[0];
            convert::type_to_schema_named(ty, defs, seen_types, Some(name))
        }
        _ => {
            // Multiple params -- wrap in an object
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();
            for (name, ty) in &params {
                properties.insert(
                    name.to_string(),
                    convert::type_to_schema_named(ty, defs, seen_types, Some(name)),
                );
                required.push(json!(name));
            }
            json!({
                "type": "object",
                "properties": Value::Object(properties),
                "required": required,
                "additionalProperties": false
            })
        }
    }
}

/// Build the outputSchema for a function.
fn build_output_schema(
    func: &wasmtime::component::types::ComponentFunc,
    defs: &mut BTreeMap<String, Value>,
    seen_types: &mut HashMap<String, usize>,
) -> Value {
    let results: Vec<_> = func.results().collect();

    match results.len() {
        0 => json!({"type": "null"}),
        1 => {
            let ty = &results[0];
            // Use result_to_output_schema for result types (simplifies result<T, string>)
            if let Type::Result(ref result_ty) = ty {
                convert::result_to_output_schema(result_ty, defs, seen_types)
            } else {
                convert::type_to_schema(ty, defs, seen_types)
            }
        }
        _ => {
            // Multiple results -- create a tuple schema
            let items: Vec<Value> = results
                .iter()
                .map(|ty| convert::type_to_schema(ty, defs, seen_types))
                .collect();
            let len = items.len();
            json!({
                "type": "array",
                "prefixItems": items,
                "minItems": len,
                "maxItems": len
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let bytes =
            std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {}: {}", path, e));
        wasmtime::component::Component::new(engine, &bytes)
            .unwrap_or_else(|e| panic!("failed to load component {}: {}", name, e))
    }

    #[test]
    fn test_echo_data_schema_has_exports_with_run() {
        let engine = make_engine();
        let component = load_component(&engine, "echo_data");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        println!(
            "echo_data schema:\n{}",
            serde_json::to_string_pretty(&schema).unwrap()
        );

        assert!(
            schema.get("exports").is_some(),
            "schema must have 'exports' key"
        );
        let exports = schema.get("exports").unwrap();
        let has_run = exports
            .as_object()
            .unwrap()
            .keys()
            .any(|k| k.contains("run"));
        assert!(
            has_run,
            "exports must contain 'run' function, got: {:?}",
            exports
        );

        let run_export = exports
            .as_object()
            .unwrap()
            .iter()
            .find(|(k, _)| k.contains("run"))
            .map(|(_, v)| v)
            .unwrap();
        assert!(
            run_export.get("inputSchema").is_some(),
            "run must have inputSchema"
        );
        assert!(
            run_export.get("outputSchema").is_some(),
            "run must have outputSchema"
        );
    }

    #[test]
    fn test_top_level_structure_d04() {
        let engine = make_engine();
        let component = load_component(&engine, "echo_data");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        assert!(
            schema.get("world").is_some(),
            "schema must have 'world' key"
        );
        assert!(
            schema.get("exports").is_some(),
            "schema must have 'exports' key"
        );
        assert!(
            schema.get("$defs").is_some(),
            "schema must have '$defs' key"
        );
    }

    #[test]
    fn test_aggregator_multiple_exports() {
        let engine = make_engine();
        let component = load_component(&engine, "timer_aggregator");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        println!(
            "timer_aggregator schema:\n{}",
            serde_json::to_string_pretty(&schema).unwrap()
        );

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

        println!(
            "square schema:\n{}",
            serde_json::to_string_pretty(&schema).unwrap()
        );

        assert!(schema.get("exports").is_some(), "schema must have exports");
    }

    #[test]
    fn test_exports_only_d05() {
        let engine = make_engine();
        let component = load_component(&engine, "echo_data");
        let schema = generate_schema(&engine, &component, &SchemaOptions::default()).unwrap();

        let exports = schema.get("exports").unwrap().as_object().unwrap();
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

        println!(
            "timer_aggregator $defs:\n{}",
            serde_json::to_string_pretty(schema.get("$defs").unwrap()).unwrap()
        );

        let defs = schema.get("$defs").unwrap().as_object().unwrap();
        assert!(
            !defs.is_empty(),
            "aggregator schema should have shared types in $defs"
        );

        let exports_str = serde_json::to_string(schema.get("exports").unwrap()).unwrap();
        assert!(
            exports_str.contains("$ref"),
            "exports should contain $ref pointers to $defs"
        );
    }

    #[test]
    fn test_generate_schema_cached_returns_cached_on_second_call() {
        let engine = make_engine();
        let path = format!(
            "{}/examples/build/components/echo_data.wasm",
            env!("CARGO_MANIFEST_DIR").replace("/packages/wit-schema", ""),
        );
        let wasm_bytes = std::fs::read(&path).unwrap();
        let component = wasmtime::component::Component::new(&engine, &wasm_bytes).unwrap();
        let cache = SchemaCache::default();
        let options = SchemaOptions::default();

        // First call should generate and cache
        let schema1 =
            generate_schema_cached(&engine, &component, &wasm_bytes, &options, &cache).unwrap();

        // Second call should return cached result
        let schema2 =
            generate_schema_cached(&engine, &component, &wasm_bytes, &options, &cache).unwrap();

        assert_eq!(schema1, schema2, "cached schema should match original");

        // Verify the digest is in the cache
        let digest = wavs_types::ComponentDigest::hash(&wasm_bytes);
        assert!(
            cache.get(&digest).is_some(),
            "cache should contain the schema"
        );
    }

    #[test]
    fn test_generate_schema_cached_different_bytes_generates_new() {
        let engine = make_engine();
        let cache = SchemaCache::default();
        let options = SchemaOptions::default();

        // Load echo_data (operator world: single "run" export)
        let echo_path = format!(
            "{}/examples/build/components/echo_data.wasm",
            env!("CARGO_MANIFEST_DIR").replace("/packages/wit-schema", ""),
        );
        let echo_bytes = std::fs::read(&echo_path).unwrap();
        let echo_component = wasmtime::component::Component::new(&engine, &echo_bytes).unwrap();

        // Load timer_aggregator (aggregator world: 3 exports)
        let agg_path = format!(
            "{}/examples/build/components/timer_aggregator.wasm",
            env!("CARGO_MANIFEST_DIR").replace("/packages/wit-schema", ""),
        );
        let agg_bytes = std::fs::read(&agg_path).unwrap();
        let agg_component = wasmtime::component::Component::new(&engine, &agg_bytes).unwrap();

        let schema1 =
            generate_schema_cached(&engine, &echo_component, &echo_bytes, &options, &cache)
                .unwrap();
        let schema2 =
            generate_schema_cached(&engine, &agg_component, &agg_bytes, &options, &cache).unwrap();

        assert_ne!(
            schema1, schema2,
            "different components should produce different schemas"
        );

        // Verify both are in the cache
        let echo_digest = wavs_types::ComponentDigest::hash(&echo_bytes);
        let agg_digest = wavs_types::ComponentDigest::hash(&agg_bytes);
        assert!(cache.get(&echo_digest).is_some(), "echo should be cached");
        assert!(
            cache.get(&agg_digest).is_some(),
            "aggregator should be cached"
        );
    }
}
