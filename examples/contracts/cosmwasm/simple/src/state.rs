use cosmwasm_std::{Order, Storage, Uint64};
use cw_storage_plus::{Bound, Map};
use anyhow::Result;

use crate::{component::ComponentOutput, entry::query::{Message, MessageWithId, MessagesResponse}};

const MESSAGES:Map<u64, Message> = Map::new("messages");

pub fn get_message(store: &dyn Storage, id: Uint64) -> Result<Message> {
    MESSAGES.load(store, id.u64()).map_err(Into::into)
}

pub fn get_messages(store: &dyn Storage, after_id: Option<Uint64>, order: Option<Order>) -> Result<MessagesResponse> {
    let after_id = after_id.map(|x| x.u64());

    let messages = MESSAGES.range(store, after_id.map(|x| Bound::exclusive(x)), None, order.unwrap_or(Order::Descending))
        .map(|x| x
            .map(|(id, msg)| MessageWithId {
                id: id.into(),
                data: msg.data,
                verified: msg.verified,
            })
            .map_err(Into::into)
        )
        .collect::<Result<_>>()?;

    Ok(MessagesResponse { messages })
}

pub fn push_message(store: &mut dyn Storage, data: Vec<u8>) -> Result<Uint64> {
    let next_index = MESSAGES.keys(store, None, None, Order::Descending).next().unwrap_or(Ok(0))? + 1;
    MESSAGES.save(store, next_index, &Message {
        data,
        verified: false,
    })?;
    Ok(next_index.into())
}

pub fn verify_message(store: &mut dyn Storage, resp: ComponentOutput) -> Result<()> {
    let message_id = resp.message_id.u64();
    let mut message = MESSAGES.load(store, message_id)?;
    if message.data != resp.message_data {
        return Err(anyhow::anyhow!("data mismatch"));
    }

    message.verified = true;
    MESSAGES.save(store, message_id, &message)?;

    Ok(())
}