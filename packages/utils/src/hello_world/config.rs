use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct HelloWorldAddresses {
    pub proxy_admin: Address,
    pub hello_world_service_manager: Address,
    pub stake_registry: Address,
    pub token: Address,
}
