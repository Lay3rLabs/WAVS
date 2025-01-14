#[allow(warnings)]
mod bindings;
use bindings::{Guest, Input};
use example_helpers::{
    query_trigger,
    trigger::{encode_trigger_output, ChainQuerierExt},
};
use layer_climb_config::{AddrKind, ChainConfig};
use layer_wasi::{cosmos::CosmosQuerier, parse_address};
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(input: Input) -> std::result::Result<Vec<u8>, String> {
        Err("TODO".to_string())
        // wstd::runtime::block_on(move |reactor| async move {
        //     let (trigger_id, req) = query_trigger!(CosmosQueryRequest, &input, reactor.clone()).await?;

        //     let querier = CosmosQuerier::new(chain_config, reactor);
        //     let resp = querier
        //         .event_trigger::<TriggerDataResp>(address, event_data)
        //         .await?;

        //     serde_json::to_vec(&Response {
        //         result: resp.data,
        //         trigger_id,
        //     })
        //     .map_err(|e| anyhow!("{:?}", e))
        //     .map(|output| encode_trigger_output(input_trigger_id, output))
        // }).map_err(|e| e.to_string())
    }
}

// The response we send back from the component, serialized to a Vec<u8>
#[derive(Serialize)]
struct Response {
    pub result: String,
    pub trigger_id: String,
}

// The response from the contract query
#[derive(Deserialize)]
struct TriggerDataResp {
    pub data: String,
}

bindings::export!(Component with_types_in bindings);
