//! WASI HTTP transport for rig agents.
//!
//! Routes all LLM API calls through wasi:http/outgoing-handler.
//!
//! Security: API key headers are never logged or printed (T-18-01).

use bytes::Bytes;
use http::{Request, Response};
use rig::http_client::{
    Error as HttpError, HttpClientExt, LazyBody, MultipartForm, Result as HttpResult,
    StreamingResponse,
};
use rig::wasm_compat::WasmCompatSend;
use wstd::http::{Body as WstdBody, Client as WstdClient};

/// Convert a wstd HTTP error (anyhow::Error) to rig's HttpError.
///
/// wstd uses `anyhow::Error` as its error type which does not implement
/// `std::error::Error` directly, so we convert via string representation.
#[inline]
fn wstd_error_to_http(e: anyhow::Error) -> HttpError {
    // Wrap the anyhow error message in a simple string error type
    #[derive(Debug)]
    struct StringError(String);
    impl std::fmt::Display for StringError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.0)
        }
    }
    impl std::error::Error for StringError {}

    HttpError::Instance(Box::new(StringError(format!("{e:#}"))))
}

/// HTTP client bridging rig's HttpClientExt to WASI outgoing HTTP.
///
/// Constructed once at agent startup and passed to the rig provider client builder.
/// All requests flow through wasi:http/outgoing-handler.
#[derive(Clone, Default)]
pub struct WasiHttpClient;

impl HttpClientExt for WasiHttpClient {
    fn send<T, U>(
        &self,
        req: Request<T>,
    ) -> impl std::future::Future<Output = HttpResult<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        T: Into<Bytes> + WasmCompatSend,
        U: From<Bytes> + WasmCompatSend + 'static,
    {
        // Convert body to Bytes and extract parts BEFORE the async block so that
        // T does not need to be 'static (the future only captures 'static data).
        let (parts, body_t) = req.into_parts();
        let body_bytes: Bytes = body_t.into();

        // Build a wstd-compatible http::Request<WstdBody>.
        // wstd re-exports http::request::Request so these are the same type.
        // We reconstruct with method, URI, and all headers — especially Authorization
        // and Content-Type which are required by LLM APIs (threat T-18-01: not logged).
        let mut builder = Request::builder()
            .method(parts.method)
            .uri(parts.uri);

        for (name, value) in parts.headers.iter() {
            builder = builder.header(name, value);
        }

        let wstd_req_result = builder
            .body(WstdBody::from(body_bytes.to_vec()))
            .map_err(HttpError::Protocol);

        async move {
            let wstd_req = wstd_req_result?;

            // Send through wasi:http/outgoing-handler via WstdClient.
            // anyhow::Error (wstd's error type) does not impl std::error::Error,
            // so we convert via a string wrapper.
            let mut response = WstdClient::new()
                .send(wstd_req)
                .await
                .map_err(wstd_error_to_http)?;

            let status = response.status();

            // Collect the full response body
            let resp_bytes = response
                .body_mut()
                .contents()
                .await
                .map_err(wstd_error_to_http)?;

            let bytes = Bytes::from(resp_bytes.to_vec());

            // Wrap the body bytes in a lazy future as required by LazyBody<U>
            let lazy_body: LazyBody<U> = Box::pin(async move { Ok(U::from(bytes)) });

            // Build the http::Response — include headers from wstd response
            let mut resp_builder = Response::builder().status(status);

            if let Some(headers_mut) = resp_builder.headers_mut() {
                *headers_mut = response.headers().clone();
            }

            resp_builder.body(lazy_body).map_err(HttpError::Protocol)
        }
    }

    fn send_multipart<U>(
        &self,
        _req: Request<MultipartForm>,
    ) -> impl std::future::Future<Output = HttpResult<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        U: From<Bytes> + WasmCompatSend + 'static,
    {
        async move {
            // LLM completion APIs use JSON bodies, not multipart.
            // Multipart support is out of scope for the WASM sandbox MVP.
            Err(HttpError::InvalidStatusCode(http::StatusCode::NOT_IMPLEMENTED))
        }
    }

    fn send_streaming<T>(
        &self,
        _req: Request<T>,
    ) -> impl std::future::Future<Output = HttpResult<StreamingResponse>> + WasmCompatSend
    where
        T: Into<Bytes>,
    {
        async move {
            // Streaming is out of scope per REQUIREMENTS.md.
            // WASI sandbox does not expose incremental response streaming.
            Err(HttpError::StreamEnded)
        }
    }
}
