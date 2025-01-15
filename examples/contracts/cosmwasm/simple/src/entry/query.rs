use anyhow::Result;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{entry_point, to_json_binary, Deps, Env, Order, QueryResponse, Uint64};

use crate::state::{get_message, get_messages};

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(Message)]
    GetMessage { id: Uint64 },

    #[returns(MessagesResponse)]
    GetMessages {
        after_id: Option<Uint64>,
        // default is [Order::Descending]
        order: Option<Order>,
    },
}

#[cw_serde]
pub struct Message {
    pub data: Vec<u8>,
    pub verified: bool,
}

#[cw_serde]
pub struct MessageWithId {
    pub id: Uint64,
    pub data: Vec<u8>,
    pub verified: bool,
}

/// Response for [QueryMsg::GetMessages]
#[cw_serde]
pub struct MessagesResponse {
    pub messages: Vec<MessageWithId>,
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<QueryResponse> {
    match msg {
        QueryMsg::GetMessage { id } => to_json_binary(&get_message(deps.storage, id)?),
        QueryMsg::GetMessages { after_id, order } => {
            to_json_binary(&get_messages(deps.storage, after_id, order)?)
        }
    }
    .map_err(|e| e.into())
}
