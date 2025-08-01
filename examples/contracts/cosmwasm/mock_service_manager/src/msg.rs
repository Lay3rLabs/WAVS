use cosmwasm_schema::cw_serde;
use layer_climb_address::AddrEvm;
use wavs_types::contracts::cosmwasm::service_manager::{
    ServiceManagerExecuteMessages, ServiceManagerQueryMessages,
};

#[cw_serde]
pub enum ExecuteMsg {
    Wavs(ServiceManagerExecuteMessages),
    SetSigningKey {
        operator: AddrEvm,
        signing_key: AddrEvm,
    },
}

#[cw_serde]
pub enum QueryMsg {
    #[serde(untagged)]
    Wavs(ServiceManagerQueryMessages),
}
