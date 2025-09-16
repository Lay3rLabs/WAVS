use anyhow::Result;
use example_helpers::bindings::world::{
    wasi::keyvalue::{atomics, store},
    wavs::{
        operator::{
            input::{Trigger, TriggerAction, TriggerData},
            output::WasmResponse,
        },
        types::{events::TriggerDataBlockInterval, service::TriggerBlockInterval},
    },
    Guest,
};
use example_helpers::export_layer_trigger_world;
use example_helpers::trigger::encode_trigger_output;
use example_types::BlockIntervalResponse;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Option<WasmResponse>, String> {
        // hardcoding this because our tests are mostly about event-based triggers
        // but this component is not event-based
        let trigger_id = 1337;

        match (trigger_action.config.trigger, trigger_action.data) {
            (Trigger::BlockInterval(config), TriggerData::BlockInterval(data)) => {
                if let Some(resp) = inner_run_task(config, data).map_err(|e| e.to_string())? {
                    let resp = serde_json::to_vec(&resp).map_err(|e| e.to_string())?;
                    Ok(Some(encode_trigger_output(trigger_id, resp)))
                } else {
                    Ok(None)
                }
            }
            _ => Err("Invalid trigger config or data".to_string()),
        }
    }
}

fn inner_run_task(
    config: TriggerBlockInterval,
    data: TriggerDataBlockInterval,
) -> Result<Option<BlockIntervalResponse>> {
    let bucket = store::open("foo")?;
    let count = atomics::increment(&bucket, "bar", 1)?.try_into()?;

    // TIP: temporarily comment out the `config.start_block.is_none()` condition
    // but leave the `count == 1`
    // and recompile to confirm that the trigger is deleted after the first trigger
    // (test will hang because trigger is deleted as a one-shot, but we haven't returned a response yet)
    if count == 1 && config.start_block.is_none() {
        // If this is the first trigger and no start block is set, wait for the next trigger
        Ok(None)
    } else {
        Ok(Some(BlockIntervalResponse {
            trigger_config_start: config.start_block,
            trigger_config_end: config.end_block,
            trigger_config_interval: config.n_blocks,
            trigger_data_block_height: data.block_height,
            count,
        }))
    }
}

export_layer_trigger_world!(Component);
