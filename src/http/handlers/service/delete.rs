use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    apis::ID,
    http::{error::HttpResult, state::HttpState},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteApps {
    pub apps: Vec<String>,
}

#[axum::debug_handler]
pub async fn handle_delete_service(
    State(state): State<HttpState>,
    Json(req): Json<DeleteApps>,
) -> impl IntoResponse {
    match delete_service_inner(state, req.apps).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}

async fn delete_service_inner(state: HttpState, app_names: Vec<String>) -> HttpResult<()> {
    for app_name in app_names {
        state.dispatcher.remove_service(ID::new(&app_name)?)?;
    }

    Ok(())
}
