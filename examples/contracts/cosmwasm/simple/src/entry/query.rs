use anyhow::Result;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{entry_point, to_json_binary, Deps, Env, Order, QueryResponse, Uint64};

use crate::state::{get_message, get_messages};

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(MessagesResponse)]
    GetMessages {
        after_id: Option<Uint64>,
        // default is [Order::Descending]
        order: Option<Order>,
    },

    // for generic helper impl
    #[returns(TriggerResponse)]
    TriggerData { trigger_id: String },
}

#[cw_serde]
pub struct MessageWithId {
    pub id: Uint64,
    pub data: Vec<u8>,
}

/// Response for [QueryMsg::GetMessages]
#[cw_serde]
pub struct MessagesResponse {
    pub messages: Vec<MessageWithId>,
}

/// Response for [QueryMsg::TriggerData]
#[cw_serde]
pub struct TriggerResponse {
    pub data: Vec<u8>,
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<QueryResponse> {
    match msg {
        QueryMsg::GetMessages { after_id, order } => {
            to_json_binary(&get_messages(deps.storage, after_id, order)?)
        }
        QueryMsg::TriggerData { trigger_id } => {
            let message = get_message(deps.storage, trigger_id.parse::<u64>()?.into())?;
            to_json_binary(&TriggerResponse { data: message.data })
        }
    }
    .map_err(|e| anyhow::anyhow!("Failed to serialize response: {}", e))
}
