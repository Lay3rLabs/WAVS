use axum::{body::Bytes, extract::State, response::IntoResponse, Json};

use crate::http::state::HttpState;

#[axum::debug_handler]
pub async fn handle_upload_service(
    State(_state): State<HttpState>,
    _bytes: Bytes,
) -> impl IntoResponse {
    Json::<[(); 0]>([]).into_response()
}
