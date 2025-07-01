// I'm compiling new bindings because of the new wit files I'm testing for kv store
wit_bindgen::generate!({
    world: "layer-trigger-world",
    path: "../../../wit",
    generate_all,
});

use wasi::keyvalue::store;

struct Counter;

impl Guest for Counter {
    fn run(_trigger_action: TriggerAction) -> Result<Option<WasmResponse>, String> {
        host::log(host::LogLevel::Info, "Counter component triggered");

        // Open the keyvalue store (empty identifier)
        let bucket = store::open("").map_err(|e| format!("Failed to open store: {:?}", e))?;

        let current_count = match bucket.get("counter") {
            Ok(Some(bytes)) => {
                let count_str = String::from_utf8_lossy(&bytes);
                count_str.parse::<u32>().unwrap_or(0)
            }
            _ => 0,
        };

        let new_count = current_count + 1;

        // Store the new count
        bucket
            .set("counter", &new_count.to_string().as_bytes().to_vec())
            .map_err(|e| format!("Failed to store counter: {:?}", e))?;
        host::log(
            host::LogLevel::Info,
            &format!(
                "Counter incremented from {} to {}",
                current_count, new_count
            ),
        );

        Ok(Some(WasmResponse {
            payload: format!("count:{}", new_count).as_bytes().to_vec(),
            ordering: None,
        }))
    }
}

export!(Counter);
