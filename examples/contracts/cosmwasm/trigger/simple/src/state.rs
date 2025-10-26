use cosmwasm_std::{HexBinary, Uint64};
use cw_storage_plus::{Item, Map};

pub const TRIGGER_MESSAGES: Map<Uint64, HexBinary> = Map::new("trigger-messages");
pub const TRIGGER_MESSAGE_COUNT: Item<u64> = Item::new("trigger-message-count");
