use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Empty, Uint256};
use layer_climb_address::AddrEvm;
use wavs_types::contracts::cosmwasm::service_manager::{
    ServiceManagerExecuteMessages, ServiceManagerQueryMessages,
};

pub type InstantiateMsg = Empty;

#[cw_serde]
#[schemaifier(mute_warnings)]
pub enum ExecuteMsg {
    /// Mock contracts get superpowers
    SetSigningKey {
        operator: AddrEvm,
        signing_key: AddrEvm,
        weight: Uint256,
    },
    #[serde(untagged)]
    Wavs(ServiceManagerExecuteMessages),
}

#[cw_serde]
#[schemaifier(mute_warnings)]
pub enum QueryMsg {
    #[serde(untagged)]
    Wavs(ServiceManagerQueryMessages),
}
