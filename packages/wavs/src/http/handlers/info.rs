use crate::http::{error::HttpResult, state::HttpState};
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct InfoResponse {
    pub message: String,
}

#[utoipa::path(
    get,
    path = "/info",
    responses(
        (status = 200, description = "Successfully retrieved service information including the list of active operators", body = InfoResponse),
        (status = 500, description = "Internal server error occurred while fetching service information")
    ),
    description = "Provides information about the WAVS service, including a list of all registered operators."
)]
#[axum::debug_handler]
pub async fn handle_info(State(state): State<HttpState>) -> impl IntoResponse {
    match inner_handle_info(state).await {
        Ok(response) => Json(response).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn inner_handle_info(_state: HttpState) -> HttpResult<InfoResponse> {
    // TODO: could do things like return operator as address for each configured chain
    // for now just return a placeholder message
    Ok(InfoResponse {
        message: "Info here :P".to_string(),
    })
}
