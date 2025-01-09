use cosmwasm_schema::cw_serde;
use cosmwasm_std::{entry_point, DepsMut, Env, MessageInfo, Response};
use anyhow::Result;
use layer_cosmwasm::event::LayerTriggerEvent;

use crate::{component::ComponentInput, event::NewMessageEvent, state::{push_message, verify_message}};

#[cw_serde]
pub enum ExecuteMsg {
    // Required by Layer
    LayerSubmit {
        // Deserializes to ComponentResponse
        data: Vec<u8>,
        // Not doing anything with this yet... commitments :)
        signature: Vec<u8>
    },

    // Proprietary per-app... but will emit a layer_cosmwasm::event::LayerTriggerEvent with MessageResponse in the data
    // A contract is expected to handle that trigger and return a ComponentResponse
    AddMessage {
        data: Vec<u8>,
        // if false (default), will not emit the message data in the LayerTriggerEvent
        // rather, it will only emit the id, and the component is expected to query 
        layer_emit_message_data: Option<bool>
    }
}


#[entry_point]
pub fn execute(deps: DepsMut, _env: Env, _info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::LayerSubmit { data, .. } => {
            let component_output = cosmwasm_std::from_json(data)?;

            verify_message(deps.storage, component_output)?;

            Ok(Response::default())
        }

        ExecuteMsg::AddMessage { data, layer_emit_message_data } => {
            let id = push_message(deps.storage, data.clone())?;

            Ok(Response::default()
                .add_event(NewMessageEvent { 
                    id,
                    data: data.clone()
                })
                .add_event(LayerTriggerEvent { 
                    data: cosmwasm_std::to_json_vec(&ComponentInput {
                        message_id: id.into(),
                        message_data: match layer_emit_message_data.unwrap_or_default() {
                            true => Some(data),
                            false => None
                        }
                    })?
                })
            )
        }
    }
}