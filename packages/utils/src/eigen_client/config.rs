use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EigenClientConfig {
    pub core: CoreDeploymentConfig,
    pub avs: AvsDeploymentConfig,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CoreDeploymentConfig {
    pub last_update: LastUpdateConfig,
    pub addresses: CoreAddressesConfig,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AvsDeploymentConfig {
    pub last_update: LastUpdateConfig,
    pub addresses: AvsAddressesConfig,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LastUpdateConfig {
    timestamp: String,
    block_number: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CoreAddressesConfig {
    pub proxy_admin: Address,
    pub delegation: Address,
    pub delegation_manager_impl: Address,
    pub strategy_manager: Address,
    pub strategy_manager_impl: Address,
    pub eigen_pod_manager: Address,
    pub eigen_pod_manager_impl: Address,
    pub strategy_factory: Address,
    pub strategy_factory_impl: Address,
    pub strategy_beacon: Address,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AvsAddressesConfig {
    pub proxy_admin: Address,
    // TODO - make this dynamic, not hello-world, etc.
    pub hello_world_service_manager: Address,
    pub hello_world_service_manager_impl: Address,
    pub stake_registry: Address,
    pub stake_registry_impl: Address,
    pub strategy: Address,
    pub token: Address,
}
