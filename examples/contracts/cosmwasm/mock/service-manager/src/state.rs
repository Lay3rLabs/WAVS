use cosmwasm_std::Uint256;
use cw_storage_plus::{Item, Map};
use layer_climb_address::AddrEvm;

pub const SERVICE_URI: Item<String> = Item::new("service-uri");
pub const OPERATOR_SIGNING_KEY_ADDRS: Map<&AddrEvm, AddrEvm> =
    Map::new("operator-signing-key-addrs");
pub const OPERATOR_WEIGHTS: Map<&AddrEvm, Uint256> = Map::new("operator-weight");
