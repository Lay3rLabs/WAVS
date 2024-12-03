use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct CoreAVSAddresses {
    pub proxy_admin: Address,
    pub delegation_manager: Address,
    pub strategy_manager: Address,
    pub eigen_pod_manager: Address,
    pub eigen_pod_beacon: Address,
    pub pauser_registry: Address,
    pub strategy_factory: Address,
    pub strategy_beacon: Address,
    pub avs_directory: Address,
    pub rewards_coordinator: Address,
}
