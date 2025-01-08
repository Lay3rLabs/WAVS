#[allow(warnings)]
mod bindings;
use bindings::{Contract, Guest};
use layer_climb_config::{AddrKind, ChainConfig};
use serde::{Deserialize, Serialize};
use wavs_helpers::{cosmos::CosmosQuerier, parse_address};

struct Component;

impl Guest for Component {
    fn run(contract: Contract, event_data: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        let address = parse_address!(bindings::lay3r::avs::wavs_types::Address, contract.address);

        let chain_config = match contract.chain_id.as_str() {
            "local-osmosis" => ChainConfig {
                chain_id: contract.chain_id.parse().unwrap(),
                rpc_endpoint: Some("http://127.0.0.1:26657".to_string()),
                grpc_endpoint: None,
                grpc_web_endpoint: None,
                gas_price: 0.025,
                gas_denom: "uosmo".to_string(),
                address_kind: AddrKind::Cosmos {
                    prefix: "osmo".to_string(),
                },
            },
            _ => {
                return Err(format!("Unsupported chain_id: {}", contract.chain_id));
            }
        };

        let (trigger_id, resp) = wstd::runtime::block_on(|reactor| async move {
            let querier = CosmosQuerier::new(chain_config, reactor);
            querier
                .event_trigger::<TriggerDataResp>(address, event_data)
                .await
                .map_err(|e| e.to_string())
        })?;

        serde_json::to_vec(&Response {
            result: resp.data,
            trigger_id,
        })
        .map_err(|e| e.to_string())
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
