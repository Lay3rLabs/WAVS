use axum::{body::Bytes, extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{http::state::HttpState, Digest};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadServiceResponse {
    pub digest: Digest,
}

#[axum::debug_handler]
pub async fn handle_upload_service(
    State(_state): State<HttpState>,
    bytes: Bytes,
) -> impl IntoResponse {
    let digest = Digest::new(&bytes);

    Json(UploadServiceResponse { digest }).into_response()
}
