use crate::http::{error::HttpResult, state::HttpState};
use axum::{body::Bytes, extract::State, response::IntoResponse, Json};
use wavs_types::UploadComponentResponse;

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
    let digest = tokio::task::spawn_blocking(|| async move {
        state.dispatcher.store_component_bytes(bytes.to_vec())
    })
    .await
    .unwrap()
    .await?
    .into();

    Ok(UploadComponentResponse { digest })
}
