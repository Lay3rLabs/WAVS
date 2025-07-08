use anyhow::Result;
use cosmwasm_std::{entry_point, DepsMut, Empty, Env, MessageInfo, Response};
use cw2::set_contract_version;

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(deps: DepsMut, _env: Env, _info: MessageInfo, _msg: Empty) -> Result<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION).map_err(|e| anyhow::anyhow!("Failed to set contract version: {}", e))?;

    Ok(Response::default())
}
