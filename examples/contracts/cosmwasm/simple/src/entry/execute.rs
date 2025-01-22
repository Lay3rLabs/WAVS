use anyhow::Result;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{entry_point, DepsMut, Env, MessageInfo, Response};

use crate::{event::NewMessageEvent, state::push_message};

#[cw_serde]
pub enum ExecuteMsg {
    // WAVS is expected to handle the event emitted from here
    AddMessage { data: Vec<u8> },
}

#[entry_point]
pub fn execute(deps: DepsMut, _env: Env, _info: MessageInfo, msg: ExecuteMsg) -> Result<Response> {
    match msg {
        ExecuteMsg::AddMessage { data } => {
            let id = push_message(deps.storage, data.clone())?;

            Ok(Response::default().add_event(NewMessageEvent {
                id,
                data: data.clone(),
            }))
        }
    }
}
