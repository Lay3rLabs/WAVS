use axum::{extract::State, response::IntoResponse, Json};

use crate::{config::Config, http::state::HttpState};

#[utoipa::path(
    get,
    path = "/config",
    responses(
        (status = 200, description = "Successfully retrieved configuration", body = Config),
        (status = 500, description = "Internal server error occurred while fetching configuration")
    ),
    description = "Returns the current configuration settings for the aggregator service"
)]
#[axum::debug_handler]
pub async fn handle_config(State(state): State<HttpState>) -> impl IntoResponse {
    Json(state.config).into_response()
}
