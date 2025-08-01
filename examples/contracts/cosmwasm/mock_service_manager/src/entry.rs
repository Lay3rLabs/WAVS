use cosmwasm_std::{
    entry_point, to_json_binary, Deps, DepsMut, Empty, Env, MessageInfo, QueryResponse, Response,
    StdResult, Uint256,
};
use cw2::set_contract_version;
use layer_climb_address::AddrEvm;
use wavs_types::contracts::cosmwasm::service_manager::{
    ServiceManagerExecuteMessages, ServiceManagerQueryMessages, WavsServiceUriUpdatedEvent,
    WavsValidateError, WavsValidateResult,
};

use crate::{
    msg::{ExecuteMsg, QueryMsg},
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
        ExecuteMsg::Wavs(msg) => match msg {
            ServiceManagerExecuteMessages::WavsSetServiceUri { service_uri } => {
                state::SERVICE_URI.save(deps.storage, &service_uri)?;

                Ok(Response::new().add_event(WavsServiceUriUpdatedEvent { service_uri }))
            }
        },
        ExecuteMsg::SetSigningKey {
            operator,
            signing_key,
        } => {
            // Logic to set the signing key for the operator
            // This is a placeholder as the actual logic will depend on your application requirements
            state::OPERATOR_SIGNING_KEY_ADDRS.save(
                deps.storage,
                operator.as_bytes(),
                &signing_key.as_bytes(),
            )?;
            Ok(Response::default())
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::Wavs(msg) => match msg {
            ServiceManagerQueryMessages::WavsOperatorWeight { operator_address } => {
                // TODO: query stake registry etc.
                to_json_binary(
                    &state::OPERATOR_WEIGHTS.load(deps.storage, operator_address.as_bytes())?,
                )
            }
            ServiceManagerQueryMessages::WavsValidate {
                envelope: _,
                signature_data,
            } => {
                // TODO: real validation logic
                if signature_data.signatures.is_empty() {
                    to_json_binary(&WavsValidateResult::Err(
                        WavsValidateError::InvalidSignatureLength,
                    ))
                } else {
                    to_json_binary(&WavsValidateResult::Ok)
                }
            }
            ServiceManagerQueryMessages::WavsServiceUri {} => {
                to_json_binary(&state::SERVICE_URI.load(deps.storage)?)
            }
            ServiceManagerQueryMessages::WavsLatestOperatorForSigningKey { signing_key_addr } => {
                match state::OPERATOR_SIGNING_KEY_ADDRS
                    .load(deps.storage, signing_key_addr.as_bytes())
                {
                    Ok(addr_bytes) => {
                        let addr = AddrEvm::new(addr_bytes);
                        to_json_binary(&addr)
                    }
                    Err(_) => to_json_binary(&Option::<AddrEvm>::None),
                }
            }
        },
    }
}
