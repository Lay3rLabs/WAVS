use std::fs;
use std::path::{Path, PathBuf};

/// Returns the main WAVS WIT interface definitions.
/// Used by `wavs_get_wit_interface` to give AI assistants full knowledge of
/// available WASM APIs (HTTP, KV, sockets, TLS, host functions, etc.).
pub fn get_wit_interface() -> String {
    let operator = include_str!("../../../wit-definitions/operator/wit/operator.wit");
    let core = include_str!("../../../wit-definitions/types/wit/core.wit");
    let service = include_str!("../../../wit-definitions/types/wit/service.wit");
    let events = include_str!("../../../wit-definitions/types/wit/events.wit");
    let chain = include_str!("../../../wit-definitions/types/wit/chain.wit");

    format!(
        "# WAVS WIT Interface Definitions\n\n\
         ## operator.wit (main world — implement this)\n\
         ```wit\n{operator}\n```\n\n\
         ## types/core.wit\n\
         ```wit\n{core}\n```\n\n\
         ## types/service.wit\n\
         ```wit\n{service}\n```\n\n\
         ## types/events.wit\n\
         ```wit\n{events}\n```\n\n\
         ## types/chain.wit\n\
         ```wit\n{chain}\n```"
    )
}

/// All WIT dependency files bundled at compile time.
const WIT_DEPS: &[(&str, &str)] = &[
    (
        "wasi-cli-0.2.0",
        include_str!("../../../wit-definitions/operator/wit/deps/wasi-cli-0.2.0/package.wit"),
    ),
    (
        "wasi-clocks-0.2.0",
        include_str!("../../../wit-definitions/operator/wit/deps/wasi-clocks-0.2.0/package.wit"),
    ),
    (
        "wasi-filesystem-0.2.0",
        include_str!(
            "../../../wit-definitions/operator/wit/deps/wasi-filesystem-0.2.0/package.wit"
        ),
    ),
    (
        "wasi-http-0.2.0",
        include_str!("../../../wit-definitions/operator/wit/deps/wasi-http-0.2.0/package.wit"),
    ),
    (
        "wasi-io-0.2.0",
        include_str!("../../../wit-definitions/operator/wit/deps/wasi-io-0.2.0/package.wit"),
    ),
    (
        "wasi-keyvalue-0.2.0-draft2",
        include_str!(
            "../../../wit-definitions/operator/wit/deps/wasi-keyvalue-0.2.0-draft2/package.wit"
        ),
    ),
    (
        "wasi-random-0.2.0",
        include_str!("../../../wit-definitions/operator/wit/deps/wasi-random-0.2.0/package.wit"),
    ),
    (
        "wasi-sockets-0.2.0",
        include_str!("../../../wit-definitions/operator/wit/deps/wasi-sockets-0.2.0/package.wit"),
    ),
    (
        "wasi-tls-0.2.0-draft",
        include_str!("../../../wit-definitions/operator/wit/deps/wasi-tls-0.2.0-draft/package.wit"),
    ),
    (
        "wavs-types-2.7.0",
        include_str!("../../../wit-definitions/operator/wit/deps/wavs-types-2.7.0/package.wit"),
    ),
];

const OPERATOR_WIT: &str = include_str!("../../../wit-definitions/operator/wit/operator.wit");

// ---------------------------------------------------------------------------
// Return scaffold as text (no disk writes)
// ---------------------------------------------------------------------------

/// Return all file contents as a formatted text block for the agent to write manually.
pub fn scaffold_component_text(
    name: &str,
    trigger_type: &str,
    description: Option<&str>,
) -> String {
    let desc = description.unwrap_or("A WAVS WASM component");
    let underscored = name.replace('-', "_");

    let cargo_toml = generate_cargo_toml(name);
    let lib_rs = generate_lib_rs(trigger_type, desc);

    let mut wit_sections = String::new();
    wit_sections.push_str(&format!(
        "### `{name}/wit/operator.wit`\n```wit\n{OPERATOR_WIT}\n```\n\n"
    ));
    for (dep_name, content) in WIT_DEPS {
        wit_sections.push_str(&format!(
            "### `{name}/wit/deps/{dep_name}/package.wit`\n```wit\n{content}\n```\n\n"
        ));
    }

    format!(
        "# Scaffold: `{name}` ({trigger_type})\n\n\
         {desc}\n\n\
         **Write ALL files below exactly as shown.** The WIT files and bindings.rs must not be modified.\n\
         Build with: `cargo build --target wasm32-wasip2 --release`\n\
         Prerequisite: `rustup target add wasm32-wasip2`\n\n\
         > **Tip:** Call this tool again with `dir` parameter to write files to disk automatically.\n\n\
         ## Directory structure\n\
         ```\n\
         {name}/\n\
         ├── Cargo.toml\n\
         ├── src/\n\
         │   ├── lib.rs\n\
         │   └── bindings.rs\n\
         └── wit/\n\
             ├── operator.wit\n\
             └── deps/ (10 packages)\n\
         ```\n\n\
         ### `{name}/Cargo.toml`\n\
         ```toml\n{cargo_toml}```\n\n\
         ### `{name}/src/lib.rs`\n\
         ```rust\n{lib_rs}```\n\n\
         ### `{name}/src/bindings.rs`\n\
         ```rust\n{BINDINGS_RS}```\n\n\
         {wit_sections}\
         ## Build\n\
         ```bash\n\
         cd {name}\n\
         cargo build --target wasm32-wasip2 --release\n\
         # Output: target/wasm32-wasip2/release/{underscored}.wasm\n\
         ```\n",
    )
}

// ---------------------------------------------------------------------------
// Write scaffold to disk
// ---------------------------------------------------------------------------

/// Create a complete, self-contained WAVS component project on disk.
///
/// Writes all files needed to build immediately:
/// - `Cargo.toml` with direct dependencies (no workspace)
/// - `src/lib.rs` with trigger-specific template code
/// - `src/bindings.rs` with wit-bindgen generation
/// - `wit/operator.wit` and all `wit/deps/*/package.wit` files
///
/// Returns a summary string describing what was created and how to build.
pub fn scaffold_component_to_disk(
    name: &str,
    trigger_type: &str,
    parent_dir: &str,
    description: Option<&str>,
) -> Result<String, String> {
    let desc = description.unwrap_or("A WAVS WASM component");
    let project_dir = PathBuf::from(parent_dir).join(name);

    if project_dir.exists() {
        return Err(format!(
            "Directory already exists: {}. Remove it first or choose a different name.",
            project_dir.display()
        ));
    }

    // Create directory structure
    let src_dir = project_dir.join("src");
    let wit_dir = project_dir.join("wit");
    let wit_deps_dir = wit_dir.join("deps");

    create_dir(&src_dir)?;
    for (dep_name, _) in WIT_DEPS {
        create_dir(&wit_deps_dir.join(dep_name))?;
    }

    // Write Cargo.toml
    write_file(&project_dir.join("Cargo.toml"), &generate_cargo_toml(name))?;

    // Write src/lib.rs
    write_file(
        &src_dir.join("lib.rs"),
        &generate_lib_rs(trigger_type, desc),
    )?;

    // Write src/bindings.rs
    write_file(&src_dir.join("bindings.rs"), BINDINGS_RS)?;

    // Write wit/operator.wit
    write_file(&wit_dir.join("operator.wit"), OPERATOR_WIT)?;

    // Write all WIT dependency files
    for (dep_name, content) in WIT_DEPS {
        write_file(&wit_deps_dir.join(dep_name).join("package.wit"), content)?;
    }

    let underscored = name.replace('-', "_");
    let abs_path = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.clone());

    Ok(format!(
        "# ✅ Component `{name}` created successfully\n\n\
         **Location:** `{path}`\n\n\
         ## Files written\n\
         ```\n\
         {name}/\n\
         ├── Cargo.toml\n\
         ├── src/\n\
         │   ├── lib.rs          ← your component logic (customize this)\n\
         │   └── bindings.rs     ← auto-generated WAVS bindings (do not edit)\n\
         └── wit/                ← WAVS interface definitions (do not edit)\n\
             ├── operator.wit\n\
             └── deps/ (10 packages)\n\
         ```\n\n\
         ## Next steps\n\n\
         1. **Customize** `src/lib.rs` with your component logic\n\
         2. **Build:** `wavs_build_component` with dir=`{path}`\n\
         3. **Validate:** `wavs_validate_component` with wasm_path=`{path}/target/wasm32-wasip2/release/{underscored}.wasm`\n\
         4. **Upload:** `wavs_upload_component` with the .wasm path\n\
         5. **Deploy:** `wavs_deploy_dev_service` with the returned digest\n\
         6. **Test:** `wavs_simulate_trigger` to verify\n\n\
         ## Build command (manual)\n\
         ```bash\n\
         cd {path}\n\
         rustup target add wasm32-wasip2  # one-time setup\n\
         cargo build --target wasm32-wasip2 --release\n\
         ```\n\n\
         ## Trigger type: `{trigger_type}`\n\
         The generated `src/lib.rs` handles `{trigger_type}` triggers.\n\
         Edit the `match action.data` block to implement your logic.\n",
        path = abs_path.display(),
        underscored = underscored,
        trigger_type = trigger_type,
    ))
}

fn create_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|e| format!("Failed to create directory {}: {e}", path.display()))
}

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// File templates
// ---------------------------------------------------------------------------

fn generate_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
wit-bindgen = {{ version = "0.53.1", features = ["bitflags"] }}
wit-bindgen-rt = {{ version = "0.44.0", features = ["bitflags"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
anyhow = "1"

[lib]
crate-type = ["cdylib"]

[profile.release]
codegen-units = 1
opt-level = "s"
debug = false
strip = true
lto = true
"#
    )
}

const BINDINGS_RS: &str = r#"#[allow(warnings)]
mod _inner {
    wit_bindgen::generate!({
        world: "wavs-world",
        path: "wit",
        pub_export_macro: true,
        generate_all,
        features: ["tls"],
    });
}
pub use _inner::*;
"#;

fn generate_lib_rs(trigger_type: &str, desc: &str) -> String {
    let (imports, body) = trigger_match_code(trigger_type);

    format!(
        r#"// {desc}
#[allow(warnings)]
mod bindings;

{imports}

struct Component;
bindings::export!(Component with_types_in bindings);

impl Guest for Component {{
    fn run(action: TriggerAction) -> std::result::Result<Vec<WasmResponse>, String> {{
{body}
    }}
}}
"#,
    )
}

// ---------------------------------------------------------------------------
// Trigger-specific code generation
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaffold_to_disk_and_build() {
        let tmp = std::env::temp_dir().join("wavs-scaffold-test");
        if tmp.exists() {
            fs::remove_dir_all(&tmp).unwrap();
        }
        fs::create_dir_all(&tmp).unwrap();

        // Test each trigger type scaffolds without error
        for trigger in &[
            "manual",
            "cron",
            "block_interval",
            "evm_contract_event",
            "cosmos_contract_event",
        ] {
            let name = format!("test-{}", trigger.replace('_', "-"));
            let result = scaffold_component_to_disk(
                &name,
                trigger,
                tmp.to_str().unwrap(),
                Some("Test component"),
            );
            assert!(
                result.is_ok(),
                "scaffold failed for {trigger}: {}",
                result.unwrap_err()
            );

            let project = tmp.join(&name);
            assert!(
                project.join("Cargo.toml").exists(),
                "missing Cargo.toml for {trigger}"
            );
            assert!(
                project.join("src/lib.rs").exists(),
                "missing lib.rs for {trigger}"
            );
            assert!(
                project.join("src/bindings.rs").exists(),
                "missing bindings.rs for {trigger}"
            );
            assert!(
                project.join("wit/operator.wit").exists(),
                "missing operator.wit for {trigger}"
            );
            assert!(
                project
                    .join("wit/deps/wavs-types-2.7.0/package.wit")
                    .exists(),
                "missing wavs-types for {trigger}"
            );

            // Verify 10 WIT dep directories
            let deps: Vec<_> = fs::read_dir(project.join("wit/deps")).unwrap().collect();
            assert_eq!(
                deps.len(),
                10,
                "expected 10 WIT deps for {trigger}, got {}",
                deps.len()
            );
        }

        // Verify duplicate directory is rejected
        let dup = scaffold_component_to_disk("test-manual", "manual", tmp.to_str().unwrap(), None);
        assert!(dup.is_err(), "should reject duplicate directory");

        // Clean up
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn test_scaffold_text_mode() {
        let text = scaffold_component_text("my-comp", "manual", None);
        assert!(text.contains("Cargo.toml"), "should contain Cargo.toml");
        assert!(text.contains("bindings.rs"), "should contain bindings.rs");
        assert!(text.contains("operator.wit"), "should contain operator.wit");
        assert!(
            text.contains("wavs-types-2.7.0"),
            "should contain wavs-types"
        );
        assert!(
            text.contains("wasm32-wasip2"),
            "should mention wasip2 target"
        );
    }
}

fn trigger_match_code(trigger_type: &str) -> (String, String) {
    let imports = "use crate::bindings::{\n    \
                   wavs::types::events::TriggerData,\n    \
                   Guest, TriggerAction, WasmResponse,\n\
                   };"
    .to_string();

    let body = match trigger_type {
        "cron" => "\
        match action.data {
            TriggerData::Cron(data) => {
                let timestamp_nanos = data.trigger_time.nanos;

                // TODO: Implement your cron logic here
                let output = serde_json::json!({
                    \"triggered_at_nanos\": timestamp_nanos,
                });

                let payload = serde_json::to_vec(&output)
                    .map_err(|e| e.to_string())?;

                Ok(vec![WasmResponse {
                    payload,
                    ordering: None,
                    event_id_salt: None,
                }])
            }
            _ => Err(\"Expected Cron trigger data\".to_string()),
        }"
        .to_string(),

        "block_interval" => "\
        match action.data {
            TriggerData::BlockInterval(data) => {
                let block_height = data.block_height;
                let chain = data.chain;

                // TODO: Implement your block interval logic here
                let output = serde_json::json!({
                    \"block_height\": block_height,
                    \"chain\": chain,
                });

                let payload = serde_json::to_vec(&output)
                    .map_err(|e| e.to_string())?;

                Ok(vec![WasmResponse {
                    payload,
                    ordering: None,
                    event_id_salt: None,
                }])
            }
            _ => Err(\"Expected BlockInterval trigger data\".to_string()),
        }"
        .to_string(),

        "evm_contract_event" => "\
        match action.data {
            TriggerData::EvmContractEvent(event_data) => {
                let chain = &event_data.chain;
                let log_data = &event_data.log.data.data;

                // TODO: Decode the ABI-encoded event log data
                // Use alloy-sol-types or manual ABI decoding to parse the event.
                // The raw log data bytes are in `log_data`.

                let output = serde_json::json!({
                    \"chain\": chain,
                    \"data_len\": log_data.len(),
                });

                let payload = serde_json::to_vec(&output)
                    .map_err(|e| e.to_string())?;

                Ok(vec![WasmResponse {
                    payload,
                    ordering: None,
                    event_id_salt: None,
                }])
            }
            _ => Err(\"Expected EvmContractEvent trigger data\".to_string()),
        }"
        .to_string(),

        "cosmos_contract_event" => "\
        match action.data {
            TriggerData::CosmosContractEvent(event_data) => {
                let chain = &event_data.chain;
                let event = &event_data.event;

                // TODO: Process the Cosmos contract event
                // event.ty is the event type string
                // event.attributes is a Vec of (key, value) tuples

                let output = serde_json::json!({
                    \"chain\": chain,
                    \"event_type\": event.ty,
                    \"block_height\": event_data.block_height,
                });

                let payload = serde_json::to_vec(&output)
                    .map_err(|e| e.to_string())?;

                Ok(vec![WasmResponse {
                    payload,
                    ordering: None,
                    event_id_salt: None,
                }])
            }
            _ => Err(\"Expected CosmosContractEvent trigger data\".to_string()),
        }"
        .to_string(),

        // "manual" or anything else
        _ => "\
        match action.data {
            TriggerData::Raw(data) => {
                let input = std::str::from_utf8(&data)
                    .unwrap_or(\"<non-utf8>\");

                // TODO: Implement your component logic here
                let output = serde_json::json!({
                    \"input\": input,
                    \"message\": \"Hello from the component!\",
                });

                let payload = serde_json::to_vec(&output)
                    .map_err(|e| e.to_string())?;

                Ok(vec![WasmResponse {
                    payload,
                    ordering: None,
                    event_id_salt: None,
                }])
            }
            _ => Err(\"Expected Raw trigger data (manual trigger)\".to_string()),
        }"
        .to_string(),
    };

    (imports, format!("        {body}"))
}
