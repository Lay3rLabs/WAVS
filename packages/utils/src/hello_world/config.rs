use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct HelloWorldDeployment {
    pub addresses: HelloWorldAddressesConfig,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HelloWorldAddressesConfig {
    pub proxy_admin: Address,
    pub hello_world_service_manager: Address,
    pub stake_registry: Address,
    pub token: Address,
}
