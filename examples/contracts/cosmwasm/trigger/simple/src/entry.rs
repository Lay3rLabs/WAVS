use cosmwasm_std::{
    entry_point, to_json_binary, Deps, DepsMut, Empty, Env, MessageInfo, QueryResponse, Response,
    StdResult, Uint64,
};
use cw2::set_contract_version;

use crate::state;
use cw_wavs_trigger_api::simple::{ExecuteMsg, PushMessageEvent, QueryMsg};

// version info for migration info
const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: Empty,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

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
        ExecuteMsg::Push { data } => {
            let trigger_id: u64 = state::TRIGGER_MESSAGE_COUNT
                .may_load(deps.storage)?
                .unwrap_or_default()
                + 1;

            state::TRIGGER_MESSAGE_COUNT.save(deps.storage, &trigger_id)?;

            let trigger_id = Uint64::new(trigger_id);

            state::TRIGGER_MESSAGES.save(deps.storage, trigger_id, &data)?;

            Ok(Response::new().add_event(PushMessageEvent { trigger_id, data }))
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::TriggerMessage { trigger_id } => {
            to_json_binary(&state::TRIGGER_MESSAGES.load(deps.storage, trigger_id)?)
        }
    }
}
