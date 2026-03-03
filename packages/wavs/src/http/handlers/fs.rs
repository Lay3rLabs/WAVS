use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use std::path::{Path as FsPath, PathBuf};

use crate::http::state::HttpState;

#[derive(serde::Serialize)]
pub struct FsEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

/// Prevent path traversal by ensuring the resolved path stays within base.
fn safe_join(base: &FsPath, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() {
        return Some(base.to_path_buf());
    }
    let joined = base.join(rel);
    let canonical = std::fs::canonicalize(&joined).ok()?;
    canonical.starts_with(base).then_some(canonical)
}

async fn handle_fs_inner(state: &HttpState, service_id: &str, rel_path: &str) -> impl IntoResponse {
    let base = state.config.data.join("fs").join(service_id);

    // Ensure the base exists (canonicalize requires the path to exist)
    if !base.exists() {
        return (StatusCode::NOT_FOUND, "service storage directory not found").into_response();
    }

    let canonical_base = match std::fs::canonicalize(&base) {
        Ok(p) => p,
        Err(_) => return (StatusCode::NOT_FOUND, "storage directory inaccessible").into_response(),
    };

    let target = match safe_join(&canonical_base, rel_path) {
        Some(p) => p,
        None => return (StatusCode::BAD_REQUEST, "path traversal detected").into_response(),
    };

    if !target.exists() {
        return StatusCode::NOT_FOUND.into_response();
    }

    if target.is_dir() {
        let entries: Vec<FsEntry> = match std::fs::read_dir(&target) {
            Ok(rd) => rd
                .flatten()
                .map(|entry| {
                    let meta = entry.metadata().ok();
                    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                    let size = if is_dir { None } else { meta.map(|m| m.len()) };
                    FsEntry {
                        name: entry.file_name().to_string_lossy().into_owned(),
                        is_dir,
                        size,
                    }
                })
                .collect(),
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to read directory",
                )
                    .into_response()
            }
        };
        Json(entries).into_response()
    } else {
        // Stream file bytes
        let bytes = match std::fs::read(&target) {
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
    handle_fs_inner(&state, &service_id, "").await
}

/// List directory or serve file for a service's storage (with sub-path)
pub async fn handle_fs(
    State(state): State<HttpState>,
    Path((service_id, rel_path)): Path<(String, String)>,
) -> impl IntoResponse {
    handle_fs_inner(&state, &service_id, &rel_path).await
}
