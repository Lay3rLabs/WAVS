use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use std::path::{Path as FsPath, PathBuf};
use wavs_types::ServiceId;

use crate::http::state::HttpState;

#[derive(serde::Serialize)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

/// Prevent path traversal by ensuring the resolved path stays within base.
async fn safe_join(base: &FsPath, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() {
        return Some(base.to_path_buf());
    }
    let joined = base.join(rel);
    let canonical = tokio::fs::canonicalize(&joined).await.ok()?;
    canonical.starts_with(base).then_some(canonical)
}

async fn handle_fs_inner(
    state: &HttpState,
    service_id: &ServiceId,
    rel_path: &str,
) -> impl IntoResponse {
    let base = state.config.data.join("fs").join(service_id.to_string());

    // Ensure the base exists (canonicalize requires the path to exist)
    if tokio::fs::metadata(&base).await.is_err() {
        return (StatusCode::NOT_FOUND, "service storage directory not found").into_response();
    }

    let canonical_base = match tokio::fs::canonicalize(&base).await {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, "storage directory inaccessible").into_response(),
    };

    let target = match safe_join(&canonical_base, rel_path).await {
        Some(p) => p,
        None => return (StatusCode::BAD_REQUEST, "path traversal detected").into_response(),
    };

    let meta = match tokio::fs::metadata(&target).await {
        Ok(m) => m,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    if meta.is_dir() {
        let mut read_dir = match tokio::fs::read_dir(&target).await {
            Ok(rd) => rd,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to read directory",
                )
                    .into_response()
            }
        };
        let mut entries = Vec::new();
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let meta = entry.metadata().await.ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = if is_dir { None } else { meta.map(|m| m.len()) };
            entries.push(FsEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir,
                size,
            });
        }
        Json(entries).into_response()
    } else {
        // Read file bytes into memory and return them
        let bytes = match tokio::fs::read(&target).await {
            Ok(b) => b,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "failed to read file").into_response()
            }
        };
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            Body::from(bytes),
        )
            .into_response()
    }
}

/// List root directory or serve file for a service's storage (no sub-path)
pub async fn handle_fs_root(
    State(state): State<HttpState>,
    Path(service_id): Path<String>,
) -> impl IntoResponse {
    let Ok(service_id) = service_id.parse::<ServiceId>() else {
        return (StatusCode::BAD_REQUEST, "invalid service id").into_response();
    };
    handle_fs_inner(&state, &service_id, "")
        .await
        .into_response()
}

/// List directory or serve file for a service's storage (with sub-path)
pub async fn handle_fs(
    State(state): State<HttpState>,
    Path((service_id, rel_path)): Path<(String, String)>,
) -> impl IntoResponse {
    let Ok(service_id) = service_id.parse::<ServiceId>() else {
        return (StatusCode::BAD_REQUEST, "invalid service id").into_response();
    };
    handle_fs_inner(&state, &service_id, &rel_path)
        .await
        .into_response()
}
