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

/// Generate a scaffold WASM component project.
/// Returns a formatted string containing the Cargo.toml and lib.rs for the component.
pub fn scaffold_component(name: &str, trigger_type: &str, description: Option<&str>) -> String {
    let desc = description.unwrap_or("A WAVS WASM component");
    let cargo_toml = generate_cargo_toml(name);
    let lib_rs = generate_lib_rs(name, trigger_type, desc);

    format!(
        "# Scaffold: `{name}` ({trigger_type})\n\n\
         {desc}\n\n\
         ## `Cargo.toml`\n\
         ```toml\n{cargo_toml}\n```\n\n\
         ## `src/lib.rs`\n\
         ```rust\n{lib_rs}\n```\n\n\
         ## Next steps\n\
         1. Create directory: `mkdir -p examples/components/{name}/src`\n\
         2. Write the files above\n\
         3. Add the crate to the workspace in the root `Cargo.toml`\n\
         4. Build: `cargo component build --release -p {name}`\n\
         5. Upload: use `wavs_upload_component` with the compiled `.wasm` path\n\
         6. Deploy: use `wavs_deploy_service` with the service manager address"
    )
}

fn generate_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
edition.workspace = true

[lib]
crate-type = ["cdylib"]

[package.metadata.component]
package = "wavs-user:{name}"

[dependencies]
example-helpers = {{ path = "../../_helpers" }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#
    )
}

fn generate_lib_rs(_name: &str, trigger_type: &str, description: &str) -> String {
    let trigger_comment = match trigger_type {
        "evm_contract_event" => {
            "// `data` contains the ABI-encoded EVM event log bytes.\n    \
             // Use alloy-sol-types or manual ABI decoding to parse the event."
        }
        "cosmos_contract_event" => {
            "// `data` contains the serialized Cosmos contract event bytes.\n    \
             // Deserialize using serde_json or the CosmWasm event format."
        }
        "block_interval" => {
            "// `data` contains the block height that triggered this component.\n    \
             // Decode as a u64 big-endian integer."
        }
        "cron" => {
            "// `data` contains the scheduled trigger timestamp.\n    \
             // Decode as a unix timestamp (u64 big-endian)."
        }
        _ => {
            "// `data` contains the raw trigger payload bytes.\n    \
             // The exact format depends on the trigger configuration."
        }
    };

    format!(
        r#"// {description}
use example_helpers::prelude::*;

struct Component;

impl Guest for Component {{
    fn run(action: TriggerAction) -> Result<Vec<WasmResponse>, String> {{
        let (trigger_id, data) = decode_trigger_event(action.data)?;

        {trigger_comment}

        // TODO: process `data` and compute your output
        let output = data; // echo the raw input for now

        Ok(vec![encode_trigger_output(
            trigger_id,
            &output,
            action.config.service_id,
        )])
    }}
}}

export_layer_trigger_world!(Component);
"#
    )
}
