use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::http::state::HttpState;

/// Query a value from the KV store
///
/// The key is constructed as: {service_id}/{bucket}/{key}
#[utoipa::path(
    get,
    path = "/dev/kv/{service_id}/{bucket}/{key}",
    params(
        ("service_id" = String, Path, description = "Service ID"),
        ("bucket" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Key within the bucket"),
    ),
    responses(
        (status = 200, description = "Value found", body = Vec<u8>),
        (status = 404, description = "Key not found"),
    ),
    description = "Query a value from the KV store for a specific service, bucket, and key"
)]
#[axum::debug_handler]
pub async fn handle_get_kv(
    State(state): State<HttpState>,
    Path((service_id, bucket, key)): Path<(String, String, String)>,
) -> impl IntoResponse {
    // Construct the full key: {service_id}/{bucket}/{key}
    let full_key = format!("{}/{}/{}", service_id, bucket, key);

    match state.db_storage.kv_store.get_cloned(&full_key) {
        Some(value) => (StatusCode::OK, value).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
