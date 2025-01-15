use example_helpers::trigger::query_trigger;
use layer_wasi::{
    bindings::worlds::any_contract_event::{Guest, Input},
    export_any_contract_event_world,
};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(input: Input) -> std::result::Result<Vec<u8>, String> {
        wstd::runtime::block_on(move |reactor| async move {
            let Input {
                chain_name,
                contract,
                event,
                chain_configs,
                ..
            } = input;

            let (trigger_id, resp): (u64, Vec<u8>) = query_trigger(
                &chain_name,
                &chain_configs.into(),
                contract.into(),
                event.into(),
                reactor.clone(),
            )
            .await?;

            serde_json::to_vec(&Response {
                trigger_id: trigger_id.to_string(),
                result: resp,
            })
            .map_err(|e| anyhow::anyhow!("{:?}", e))
        })
        .map_err(|e| e.to_string())
    }
}

// The response we send back from the component, serialized to a Vec<u8>
#[derive(Serialize)]
struct Response {
    pub result: Vec<u8>,
    pub trigger_id: String,
}

// The response from the contract query
#[derive(Deserialize)]
struct TriggerDataResp {
    pub data: String,
}

export_any_contract_event_world!(Component);
