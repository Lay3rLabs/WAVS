use cosmwasm_std::Addr;
use cw_storage_plus::Item;

pub const SERVICE_MANAGER: Item<Addr> = Item::new("service-manager");
