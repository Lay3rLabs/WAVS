use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use base64::Engine as _;

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

#[derive(serde::Serialize)]
pub struct KvEntry {
    pub bucket: String,
    pub key: String,
    pub value_b64: String,
}

/// List all KV entries for a given service
pub async fn handle_list_kv(
    State(state): State<HttpState>,
    Path(service_id): Path<String>,
) -> impl IntoResponse {
    let prefix = format!("{}/", service_id);
    let entries: Vec<KvEntry> = state
        .db_storage
        .kv_store
        .iter()
        .filter_map(|e| {
            let (full_key, value) = e.pair();
            let rest = full_key.strip_prefix(&prefix)?;
            let (bucket, key) = rest.split_once('/')?;
            Some(KvEntry {
                bucket: bucket.to_string(),
                key: key.to_string(),
                value_b64: base64::engine::general_purpose::STANDARD.encode(value.as_slice()),
            })
        })
        .collect();
    Json(entries).into_response()
}
