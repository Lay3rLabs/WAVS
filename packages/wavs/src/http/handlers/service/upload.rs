use crate::http::{error::HttpResult, state::HttpState};
use axum::{body::Bytes, extract::State, response::IntoResponse, Json};
use wavs_types::{Registry, ComponentSource, UploadServiceResponse};

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
) -> HttpResult<UploadServiceResponse> {
    let digest = tokio::task::spawn_blocking(|| async move {
        let source = bytes.to_vec();
        let reg: Result<Registry, serde_json::Error> = serde_json::from_slice(&source);
        match reg {
            Ok(registry) => {
                state
                    .dispatcher
                    .store_component(ComponentSource::Registry {registry}).await
            }
            _ => {
                state
                    .dispatcher
                    .store_component(ComponentSource::Bytecode(bytes.to_vec())).await
                }
    }})
    .await
    .unwrap().await?
    .into();

    Ok(UploadServiceResponse { digest })
}
