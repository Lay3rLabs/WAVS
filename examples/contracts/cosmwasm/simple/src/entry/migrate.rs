use anyhow::Result;
use cosmwasm_std::{entry_point, DepsMut, Empty, Env, Response};

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: Empty) -> Result<Response> {
    Ok(Response::default())
}
