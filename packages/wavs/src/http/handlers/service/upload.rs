use crate::http::{error::HttpResult, state::HttpState};
use axum::{body::Bytes, extract::State, response::IntoResponse, Json};
use wavs_types::UploadComponentResponse;

#[utoipa::path(
    post,
    path = "/upload",
    request_body(description = "Component file binary data (max 50MB)",
                 content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Component file uploaded successfully and stored in the system", body = UploadComponentResponse),
        (status = 400, description = "Invalid file format or corrupt data"),
        (status = 413, description = "File too large (max 50MB)"),
        (status = 500, description = "Internal server error during file processing or storage")
    ),
    description = "Uploads a component file to be used in a WAVS service. Returns a digest that can be used to reference the uploaded component in service configurations."
)]
#[axum::debug_handler]
pub async fn handle_upload_service(
    State(state): State<HttpState>,
    bytes: Bytes,
) -> impl IntoResponse {
    match inner_handle_upload_service(state, bytes).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn inner_handle_upload_service(
    state: HttpState,
    bytes: Bytes,
) -> HttpResult<UploadComponentResponse> {
    let digest =
        tokio::task::block_in_place(|| state.dispatcher.store_component_bytes(bytes.to_vec()))?;

    Ok(UploadComponentResponse { digest })
}
