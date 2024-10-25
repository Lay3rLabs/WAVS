use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    http::{state::HttpState, types::app::App},
    Digest,
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAppsResponse {
    apps: Vec<App>,
    digests: Vec<Digest>,
}

#[axum::debug_handler]
pub async fn handle_list_services(State(_state): State<HttpState>) -> impl IntoResponse {
    Json::<[(); 0]>([]).into_response()
}
