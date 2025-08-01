use cosmwasm_std::{
    entry_point, to_json_binary, Deps, DepsMut, Env, MessageInfo, QueryResponse, Response,
    StdResult,
};
use cw2::set_contract_version;
use wavs_types::contracts::cosmwasm::service_manager::ServiceManagerQueryMessages;
use wavs_types::contracts::cosmwasm::{
    service_handler::ServiceHandlerExecuteMessages, service_manager::WavsValidateResult,
};

use crate::msg::QueryMsg;
use crate::{
    msg::{ExecuteMsg, InstantiateMsg},
    state,
};

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let service_manager = deps
        .api
        .addr_validate(&msg.service_manager)
        .map_err(|_| cosmwasm_std::StdError::msg("Invalid service manager address"))?;

    state::SERVICE_MANAGER.save(deps.storage, &service_manager)?;

    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Wavs(msg) => match msg {
            ServiceHandlerExecuteMessages::WavsHandleSignedEnvelope {
                envelope,
                signature_data,
            } => {
                let contract_addr = state::SERVICE_MANAGER.load(deps.storage)?;

                let resp: WavsValidateResult = deps.querier.query_wasm_smart(
                    contract_addr,
                    &ServiceManagerQueryMessages::WavsValidate {
                        envelope,
                        signature_data,
                    },
                )?;

                if let WavsValidateResult::Err(err) = resp {
                    return Err(cosmwasm_std::StdError::from(err));
                }
            }
        },
    }

    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::ServiceManagerAddr {} => {
            to_json_binary(&state::SERVICE_MANAGER.load(deps.storage)?)
        }
    }
}
