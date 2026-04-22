//! HttpFetchTool — make HTTP requests from within the WASI sandbox.
//!
//! Uses wstd::http::Client (wasi:http/outgoing-handler) for all requests.
//! Respects AllowedHostPermission enforced by the WAVS host.

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wstd::http::{Body, Client, Request};

// ─── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum HttpFetchError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),

    #[error("HTTP body read failed: {0}")]
    BodyReadFailed(String),

    #[error("Invalid method: {0}")]
    InvalidMethod(String),
}

// ─── Types ────────────────────────────────────────────────────────────────────

/// Arguments for HttpFetchTool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HttpFetchArgs {
    /// The URL to fetch.
    pub url: String,
    /// HTTP method: GET, POST, PUT, DELETE, PATCH, HEAD. Defaults to GET.
    #[schemars(default)]
    pub method: Option<String>,
    /// Optional request body as a UTF-8 string.
    #[schemars(default)]
    pub body: Option<String>,
    /// Optional request headers as a list of [name, value] pairs.
    #[schemars(default)]
    pub headers: Option<Vec<(String, String)>>,
}

/// Response from HttpFetchTool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HttpFetchOutput {
    /// The HTTP response status code.
    pub status: u16,
    /// The response body as a UTF-8 string (lossy — invalid bytes replaced with U+FFFD).
    pub body: String,
}

// ─── HttpFetchTool ────────────────────────────────────────────────────────────

/// Make an HTTP request to a URL and return the status code and body text.
///
/// Requests flow through wasi:http/outgoing-handler. AllowedHostPermission
/// at the WAVS host level restricts which URLs are reachable.
pub struct HttpFetchTool;

impl Tool for HttpFetchTool {
    const NAME: &'static str = "http_fetch";

    type Error = HttpFetchError;
    type Args = HttpFetchArgs;
    type Output = HttpFetchOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Make an HTTP request to a URL. Returns status code and body text. \
                Respects AllowedHostPermission — the WAVS host enforces which URLs are reachable."
                .to_string(),
            parameters: serde_json::to_value(schemars::schema_for!(HttpFetchArgs))
                .unwrap_or(serde_json::Value::Null),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let method = args.method.as_deref().unwrap_or("GET").to_uppercase();

        // Build the wstd HTTP request using the correct builder method.
        let mut builder = match method.as_str() {
            "GET" => Request::get(&args.url),
            "POST" => Request::post(&args.url),
            "PUT" => Request::put(&args.url),
            "DELETE" => Request::delete(&args.url),
            "PATCH" => Request::patch(&args.url),
            "HEAD" => Request::head(&args.url),
            other => {
                return Err(HttpFetchError::InvalidMethod(other.to_string()));
            }
        };

        // Add optional headers.
        if let Some(headers) = &args.headers {
            for (name, value) in headers {
                builder = builder.header(name.as_str(), value.as_str());
            }
        }

        // Build the request with optional body.
        let body = args
            .body
            .map(|b| Body::from(b.into_bytes()))
            .unwrap_or_else(Body::empty);

        let request = builder
            .body(body)
            .map_err(|e| HttpFetchError::RequestFailed(e.to_string()))?;

        // Send through wasi:http/outgoing-handler.
        let mut response = Client::new()
            .send(request)
            .await
            .map_err(|e| HttpFetchError::RequestFailed(format!("{:#}", e)))?;

        let status = response.status().as_u16();

        let body_bytes = response
            .body_mut()
            .contents()
            .await
            .map_err(|e| HttpFetchError::BodyReadFailed(format!("{:#}", e)))?;

        let body_str = String::from_utf8_lossy(&body_bytes).into_owned();

        Ok(HttpFetchOutput {
            status,
            body: body_str,
        })
    }
}
