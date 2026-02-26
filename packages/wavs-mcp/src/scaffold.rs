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
package = "component:{name}"

[dependencies]
example-helpers = {{ workspace = true }}
"#
    )
}

fn generate_lib_rs(_name: &str, trigger_type: &str, description: &str) -> String {
    match trigger_type {
        "cron" => format!(
            r#"// {description}
use example_helpers::bindings::world::{{
    host,
    wavs::operator::{{
        input::{{TriggerAction, TriggerData}},
        output::WasmResponse,
    }},
    Guest,
}};
use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::encode_trigger_output;

struct Component;

impl Guest for Component {{
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<WasmResponse>, String> {{
        if let TriggerData::Cron(cron) = trigger_action.data {{
            // cron.trigger_time.nanos is the scheduled unix timestamp in nanoseconds
            let output = cron.trigger_time.nanos.to_be_bytes().to_vec();

            Ok(vec![encode_trigger_output(
                0,
                output,
                host::get_service().service.manager,
            )])
        }} else {{
            Err("Expected Cron trigger data".to_string())
        }}
    }}
}}

export_layer_trigger_world!(Component);
"#
        ),

        "block_interval" => format!(
            r#"// {description}
use example_helpers::bindings::world::{{
    host,
    wavs::operator::{{
        input::{{Trigger, TriggerAction, TriggerData}},
        output::WasmResponse,
    }},
    Guest,
}};
use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::encode_trigger_output;

struct Component;

impl Guest for Component {{
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<WasmResponse>, String> {{
        match (trigger_action.config.trigger, trigger_action.data) {{
            (Trigger::BlockInterval(_config), TriggerData::BlockInterval(data)) => {{
                // data.block_height is the block number that fired this trigger
                let output = data.block_height.to_be_bytes().to_vec();

                Ok(vec![encode_trigger_output(
                    0,
                    output,
                    host::get_service().service.manager,
                )])
            }}
            _ => Err("Invalid trigger data".to_string()),
        }}
    }}
}}

export_layer_trigger_world!(Component);
"#
        ),

        _ => {
            let trigger_comment = match trigger_type {
                "evm_contract_event" => {
                    "// `data` contains the ABI-encoded EVM event log bytes.\n        \
                     // Use alloy-sol-types or manual ABI decoding to parse the event."
                }
                "cosmos_contract_event" => {
                    "// `data` contains the serialized Cosmos contract event bytes.\n        \
                     // Deserialize using serde_json or the CosmWasm event format."
                }
                _ => {
                    "// `data` contains the raw trigger payload bytes.\n        \
                     // The exact format depends on the trigger configuration."
                }
            };

            format!(
                r#"// {description}
use example_helpers::bindings::world::{{
    host,
    wavs::operator::{{
        input::{{TriggerAction, TriggerData}},
        output::WasmResponse,
    }},
    Guest,
}};
use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::{{decode_trigger_event, encode_trigger_output}};

struct Component;

impl Guest for Component {{
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<WasmResponse>, String> {{
        let (trigger_id, data) = decode_trigger_event(trigger_action.data)
            .map_err(|e| e.to_string())?;

        {trigger_comment}

        // TODO: process `data` and compute your output
        let output = data;

        Ok(vec![encode_trigger_output(
            trigger_id,
            output,
            host::get_service().service.manager,
        )])
    }}
}}

export_layer_trigger_world!(Component);
"#
            )
        }
    }
}
