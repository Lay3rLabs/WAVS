use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::http::{
    state::HttpState,
    types::app::{App, Status},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppRequest {
    #[serde(flatten)]
    pub app: App,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppResponse {
    pub name: String,
    pub status: Status,
}

#[axum::debug_handler]
pub async fn handle_add_service(
    State(_state): State<HttpState>,
    Json(req): Json<RegisterAppRequest>,
) -> impl IntoResponse {
    let resp = RegisterAppResponse {
        name: req.app.name,
        status: Status::Active,
    };

    Json(resp).into_response()
}
