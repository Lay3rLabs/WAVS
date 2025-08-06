use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint64;
use wavs_types::contracts::cosmwasm::service_handler::{
    ServiceHandlerExecuteMessages, ServiceHandlerQueryMessages,
};

#[cw_serde]
pub struct InstantiateMsg {
    pub service_manager: String,
}

#[cw_serde]
#[schemaifier(mute_warnings)]
pub enum ExecuteMsg {
    #[serde(untagged)]
    Wavs(ServiceHandlerExecuteMessages),
}

#[cw_serde]
#[derive(QueryResponses)]
#[schemaifier(mute_warnings)]
pub enum QueryMsg {
    #[returns(bool)]
    TriggerValidated { trigger_id: Uint64 },

    /// Returns the abi-encoded `SignedData` for the given `trigger_id`
    #[returns(cosmwasm_std::Binary)]
    SignedData { trigger_id: Uint64 },

    #[serde(untagged)]
    #[returns(())]
    Wavs(ServiceHandlerQueryMessages),
}
