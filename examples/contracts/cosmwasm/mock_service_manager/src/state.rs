use cosmwasm_std::Uint256;
use cw_storage_plus::{Item, Map};

pub const SERVICE_URI: Item<String> = Item::new("service-uri");
pub const OPERATOR_SIGNING_KEY_ADDRS: Map<[u8; 20], [u8; 20]> =
    Map::new("operator-signing-key-addrs");
pub const OPERATOR_WEIGHTS: Map<[u8; 20], Uint256> = Map::new("operator-weight");
