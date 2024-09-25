use anyhow::Context;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

use super::Operator;
use crate::app;
use crate::digest::Digest;
use crate::storage::{Storage, StorageError};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppRequest {
    #[serde(flatten)]
    pub app: app::App,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppResponse {
    name: String,
    status: app::Status,
}

/// Adds application.
///
/// If the provided Wasm digest is not already in use, the Wasm is
/// fetched, compiled and stored.
pub async fn add<S: Storage + 'static>(
    State(operator): State<Arc<Mutex<Operator<S>>>>,
    Json(mut req): Json<RegisterAppRequest>,
) -> Result<Json<RegisterAppResponse>, AddAppError> {
    // reject if app status is defined in the request
    // TODO
    if req.app.status.is_some() {
        return Err(AddAppError::BadRequest(
            "app status should not be specified in request".to_string(),
        ));
    }

    // TODO add validation wasm

    // validate app
    req.app.validate().map_err(AddAppError::AppError)?;

    let op = operator.clone();
    let mut op = op.try_lock().or(Err(AddAppError::InternalServerError(
        "please retry".to_string(),
    )))?;

    let engine = op.engine().clone();
    let default_registry = op.registry().cloned();
    let storage = op.storage_mut();

    // check package source
    match req.app.source {
        app::Source::Uploaded { ref digest } if !storage.has_wasm(digest).await? => {
            return Err(AddAppError::DigestNotFound(digest.clone()))
        }
        app::Source::Url {
            ref url,
            ref digest,
        } if !storage.has_wasm(digest).await? => {
            let bytes = match reqwest::get(url).await {
                Ok(res) => match res.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(_) => return Err(AddAppError::DownloadFailed),
                },
                Err(_) => return Err(AddAppError::WasmNotFound),
            };

            storage
                .add_wasm(digest, &bytes, &engine)
                .await
                .map_err(AddAppError::Storage)?;
        }
        app::Source::Registry {
            ref package,
            ref mut version,
            ref mut registry,
            ref mut digest,
        } => {
            let mut config = wasm_pkg_client::Config::empty();

            // set default registry from the operator config, if defined
            config.set_default_registry(default_registry);

            // set package namespace registry, if defined for the app
            if let Some(reg) = &registry {
                config.set_namespace_registry(package.namespace().clone(), reg.clone());
            }

            // set resolved registry in app info
            if let Some(reg) = config.resolve_registry(package) {
                registry.replace(reg.clone());
            }

            // create client
            let client = wasm_pkg_client::Client::new(config);

            // get latest package version, if not specified
            let resolved_version = match &version {
                Some(ver) => ver.clone(),
                None => {
                    let versions = client
                        .list_all_versions(package)
                        .await
                        .context("listing release versions in registry")?;
                    versions
                        .into_iter()
                        .filter_map(|vi| (!vi.yanked).then_some(vi.version))
                        .max()
                        .context("No package releases found in registry")?
                }
            };

            // set resolved version in app info
            version.replace(resolved_version.clone());

            // get package release
            let release = client
                .get_release(package, &resolved_version)
                .await
                .context("get package release info from registry")?;

            // check digest
            let release_digest: Digest = release
                .content_digest
                .to_string()
                .parse()
                .context("parsing registry package content digest")?;
            // if digest is provided, verify digest matches the app request
            match digest {
                None => {
                    // use the registry digest
                    digest.replace(release_digest);
                }
                Some(digest) if digest != &release_digest => {
                    // return error if digest does not match expected
                    return Err(AddAppError::DigestMismatchWithRegistry {
                        found: release_digest,
                        expected: digest.clone(),
                    });
                }
                _ => {}
            }

            // download release
            let mut content_stream = client
                .stream_content(package, &release)
                .await
                .context("download package content stream from registry")?;

            let mut buf = Vec::new();
            while let Some(chunk) = content_stream
                .try_next()
                .await
                .context("downloading package content from registry")?
            {
                buf.extend_from_slice(&chunk);
            }

            // store the download
            storage
                .add_wasm(digest.as_ref().unwrap(), &buf, &engine)
                .await
                .map_err(AddAppError::Storage)?;
        }
        _ => {} // already has the digest available
    }

    let name = req.app.name.clone();
    storage
        .add_application(req.app)
        .await
        .map_err(AddAppError::Storage)?;

    op.activate_app(&name).await?;

    Ok(Json(RegisterAppResponse {
        name,
        status: app::Status::Active,
    }))
}

#[derive(Debug, Error)]
pub enum AddAppError {
    #[error("internal error: `{0}`")]
    InternalServerError(String),

    #[error("package digest `{0}` is not available")]
    DigestNotFound(Digest),

    #[error("Wasm URL was not found")]
    WasmNotFound,

    #[error("Wasm URL download failed")]
    DownloadFailed,

    #[error("bad request: `{0}`")]
    BadRequest(String),

    #[error("digest `{expected}` does not match the digest from the registry `{found}`")]
    DigestMismatchWithRegistry { expected: Digest, found: Digest },

    #[error("{0:?}")]
    AppError(app::AppError),

    /// An error occurred while performing a storage operation.
    #[error("{0:?}")]
    Storage(#[from] StorageError),

    /// An error occurred.
    #[error("{0:?}")]
    Other(#[from] anyhow::Error),

    /// An error occurred while performing a IO.
    #[error("error: {0:?}")]
    IoError(#[from] std::io::Error),
}

impl IntoResponse for AddAppError {
    fn into_response(self) -> Response {
        match self {
            AddAppError::Storage(_) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            AddAppError::AppError(_) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            AddAppError::WasmNotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            AddAppError::DownloadFailed => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            AddAppError::BadRequest(_) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorMessage {
                    message: self.to_string(),
                }),
            )
                .into_response(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ErrorMessage {
    message: String,
}
