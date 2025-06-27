use axum::{extract::State, response::IntoResponse, Json};
use utils::config::AnyChainConfig;
use wavs_types::AddChainResponse;

use crate::http::{error::HttpResult, state::HttpState};

#[utoipa::path(
    post,
    path = "/add-chain",
    request_body = utils::config::AnyChainConfig,
    responses(
        (status = 200, description = "Chain added successfully", body = wavs_types::AddChainResponse),
        (status = 400, description = "Invalid chain config"),
        (status = 409, description = "Chain already exists"),
        (status = 500, description = "Internal server error")
    ),
    description = "Dynamically adds a new chain configuration"
)]
#[axum::debug_handler]
pub async fn handle_add_chain(
    State(state): State<HttpState>,
    Json(chain_config): Json<AnyChainConfig>,
) -> impl IntoResponse {
    match add_chain_inner(state, chain_config).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn add_chain_inner(
    state: HttpState,
    chain_config: AnyChainConfig,
) -> HttpResult<AddChainResponse> {
    state.dispatcher.add_chain(chain_config).await?;
    Ok(AddChainResponse { success: true })
}