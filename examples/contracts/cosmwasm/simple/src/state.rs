use anyhow::Result;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Order, Storage, Uint64};
use cw_storage_plus::{Bound, Map};

use crate::entry::query::{MessageWithId, MessagesResponse};

const MESSAGES: Map<u64, Message> = Map::new("messages");

#[cw_serde]
pub struct Message {
    pub data: Vec<u8>,
}

pub fn get_message(store: &dyn Storage, id: Uint64) -> Result<Message> {
    MESSAGES.load(store, id.u64()).map_err(|e| anyhow::anyhow!(
        "Failed to load message with id {}: {}",
        id,
        e
    ))
}

pub fn get_messages(
    store: &dyn Storage,
    after_id: Option<Uint64>,
    order: Option<Order>,
) -> Result<MessagesResponse> {
    let after_id = after_id.map(|x| x.u64());

    let messages = MESSAGES
        .range(
            store,
            after_id.map(Bound::exclusive),
            None,
            order.unwrap_or(Order::Descending),
        )
        .map(|x| {
            x.map(|(id, msg)| MessageWithId {
                id: id.into(),
                data: msg.data,
            })
            .map_err(|e| {
                anyhow::anyhow!("Failed to map message: {}", e)
            })
        })
        .collect::<Result<_>>()?;

    Ok(MessagesResponse { messages })
}

pub fn push_message(store: &mut dyn Storage, data: Vec<u8>) -> Result<Uint64> {
    let next_index = MESSAGES
        .keys(store, None, None, Order::Descending)
        .next()
        .unwrap_or(Ok(0))
        .map_err(|e| anyhow::anyhow!("Failed to get next index: {}", e))
        ?
        + 1;
    MESSAGES.save(store, next_index, &Message { data }).map_err(|e| anyhow::anyhow!("failed to save message: {}", e))?;
    Ok(next_index.into())
}
