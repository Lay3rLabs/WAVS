use bytes::Bytes;
use std::future::Future;
pub use http::{HeaderMap, HeaderValue, Method, Request, Response, Uri, request::Builder};
use http::{HeaderName, StatusCode};
pub mod multipart;
pub mod retry;
// P4: SSE module gated — not available on WASM targets
#[cfg(not(target_family = "wasm"))]
pub mod sse;
use crate::wasm_compat::*;
use std::pin::Pin;

// P4: BoxedStream moved here from sse.rs so it remains accessible on all targets.
// On WASM this is the unit type placeholder; on native it is the real streaming type.
#[cfg(not(target_family = "wasm"))]
pub type BoxedStream = Pin<Box<dyn WasmCompatSendStream<InnerItem = crate::http_client::Result<Bytes>>>>;
#[cfg(target_family = "wasm")]
pub type BoxedStream = Pin<Box<dyn WasmCompatSendStream<InnerItem = crate::http_client::Result<Bytes>>>>;

pub use multipart::MultipartForm;
// P1: reqwest::Client re-export gated behind reqwest feature
#[cfg(feature = "reqwest")]
pub use reqwest::Client as ReqwestClient;

// Default HTTP client type — resolves to reqwest::Client when available, () otherwise.
// Used as the default type parameter across all provider structs to avoid
// direct reqwest::Client references that break on WASI targets.
#[cfg(feature = "reqwest")]
pub type DefaultHttpClient = reqwest::Client;
#[cfg(not(feature = "reqwest"))]
pub type DefaultHttpClient = ();

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Http error: {0}")]
    Protocol(#[from] http::Error),
    #[error("Invalid status code: {0}")]
    InvalidStatusCode(StatusCode),
    #[error("Invalid status code {0} with message: {1}")]
    InvalidStatusCodeWithMessage(StatusCode, String),
    #[error("Header value outside of legal range: {0}")]
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
    #[error("Request in error state, cannot access headers")]
    NoHeaders,
    #[error("Stream ended")]
    StreamEnded,
    #[error("Invalid content type was returned: {0:?}")]
    InvalidContentType(HeaderValue),
    #[cfg(not(target_family = "wasm"))]
    #[error("Http client error: {0}")]
    Instance(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[cfg(target_family = "wasm")]
    #[error("Http client error: {0}")]
    Instance(#[from] Box<dyn std::error::Error + 'static>),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(not(target_family = "wasm"))]
pub(crate) fn instance_error<E: std::error::Error + Send + Sync + 'static>(error: E) -> Error {
    Error::Instance(error.into())
}

#[cfg(target_family = "wasm")]
fn instance_error<E: std::error::Error + 'static>(error: E) -> Error {
    Error::Instance(error.into())
}

pub type LazyBytes = WasmBoxedFuture<'static, Result<Bytes>>;
pub type LazyBody<T> = WasmBoxedFuture<'static, Result<T>>;

pub type StreamingResponse = Response<BoxedStream>;

#[derive(Debug, Clone, Copy)]
pub struct NoBody;

impl From<NoBody> for Bytes {
    fn from(_: NoBody) -> Self {
        Bytes::new()
    }
}

// P1: reqwest::Body usage gated behind reqwest feature
#[cfg(feature = "reqwest")]
impl From<NoBody> for reqwest::Body {
    fn from(_: NoBody) -> Self {
        reqwest::Body::default()
    }
}

pub async fn text(response: Response<LazyBody<Vec<u8>>>) -> Result<String> {
    let text = response.into_body().await?;
    Ok(String::from(String::from_utf8_lossy(&text)))
}

pub fn make_auth_header(key: impl AsRef<str>) -> Result<(HeaderName, HeaderValue)> {
    Ok((
        http::header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", key.as_ref()))?,
    ))
}

pub fn bearer_auth_header(headers: &mut HeaderMap, key: impl AsRef<str>) -> Result<()> {
    let (k, v) = make_auth_header(key)?;

    headers.insert(k, v);

    Ok(())
}

pub fn with_bearer_auth(mut req: Builder, auth: &str) -> Result<Builder> {
    bearer_auth_header(req.headers_mut().ok_or(Error::NoHeaders)?, auth)?;

    Ok(req)
}

/// A helper trait to make generic requests (both regular and SSE) possible.
pub trait HttpClientExt: WasmCompatSend + WasmCompatSync {
    /// Send a HTTP request, get a response back (as bytes). Response must be able to be turned back into Bytes.
    fn send<T, U>(
        &self,
        req: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        T: Into<Bytes>,
        T: WasmCompatSend,
        U: From<Bytes>,
        U: WasmCompatSend + 'static;

    /// Send a HTTP request with a multipart body, get a response back (as bytes). Response must be able to be turned back into Bytes (although usually for the response, you will probably want to specify Bytes anyway).
    fn send_multipart<U>(
        &self,
        req: Request<MultipartForm>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        U: From<Bytes>,
        U: WasmCompatSend + 'static;

    /// Send a HTTP request, get a streamed response back (as a stream of [`bytes::Bytes`].)
    fn send_streaming<T>(
        &self,
        req: Request<T>,
    ) -> impl Future<Output = Result<StreamingResponse>> + WasmCompatSend
    where
        T: Into<Bytes>;
}

// P1: reqwest::Client impl gated behind reqwest feature
#[cfg(feature = "reqwest")]
impl HttpClientExt for reqwest::Client {
    fn send<T, U>(
        &self,
        req: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        T: Into<Bytes>,
        U: From<Bytes> + WasmCompatSend,
    {
        let (parts, body) = req.into_parts();
        let req = self
            .request(parts.method, parts.uri.to_string())
            .headers(parts.headers)
            .body(body.into());

        async move {
            let response = req.send().await.map_err(instance_error)?;
            if !response.status().is_success() {
                return Err(Error::InvalidStatusCodeWithMessage(
                    response.status(),
                    response.text().await.unwrap(),
                ));
            }

            let mut res = Response::builder().status(response.status());

            if let Some(hs) = res.headers_mut() {
                *hs = response.headers().clone();
            }

            let body: LazyBody<U> = Box::pin(async {
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|e| Error::Instance(e.into()))?;

                let body = U::from(bytes);
                Ok(body)
            });

            res.body(body).map_err(Error::Protocol)
        }
    }

    fn send_multipart<U>(
        &self,
        req: Request<MultipartForm>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        U: From<Bytes>,
        U: WasmCompatSend + 'static,
    {
        let (parts, body) = req.into_parts();
        let body = reqwest::multipart::Form::from(body);

        let req = self
            .request(parts.method, parts.uri.to_string())
            .headers(parts.headers)
            .multipart(body);

        async move {
            let response = req.send().await.map_err(instance_error)?;
            if !response.status().is_success() {
                return Err(Error::InvalidStatusCodeWithMessage(
                    response.status(),
                    response.text().await.unwrap(),
                ));
            }

            let mut res = Response::builder().status(response.status());

            if let Some(hs) = res.headers_mut() {
                *hs = response.headers().clone();
            }

            let body: LazyBody<U> = Box::pin(async {
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|e| Error::Instance(e.into()))?;

                let body = U::from(bytes);
                Ok(body)
            });

            res.body(body).map_err(Error::Protocol)
        }
    }

    fn send_streaming<T>(
        &self,
        req: Request<T>,
    ) -> impl Future<Output = Result<StreamingResponse>> + WasmCompatSend
    where
        T: Into<Bytes>,
    {
        let (parts, body) = req.into_parts();

        let req = self
            .request(parts.method, parts.uri.to_string())
            .headers(parts.headers)
            .body(body.into())
            .build()
            .map_err(|x| Error::Instance(x.into()))
            .unwrap();

        let client = self.clone();

        async move {
            let response: reqwest::Response = client.execute(req).await.map_err(instance_error)?;
            if !response.status().is_success() {
                return Err(Error::InvalidStatusCodeWithMessage(
                    response.status(),
                    response.text().await.unwrap(),
                ));
            }

            #[cfg(not(target_family = "wasm"))]
            let mut res = Response::builder()
                .status(response.status())
                .version(response.version());

            #[cfg(target_family = "wasm")]
            let mut res = Response::builder().status(response.status());

            if let Some(hs) = res.headers_mut() {
                *hs = response.headers().clone();
            }

            use futures::StreamExt;

            let mapped_stream: Pin<Box<dyn WasmCompatSendStream<InnerItem = Result<Bytes>>>> =
                Box::pin(
                    response
                        .bytes_stream()
                        .map(|chunk| chunk.map_err(|e| Error::Instance(Box::new(e)))),
                );

            res.body(mapped_stream).map_err(Error::Protocol)
        }
    }
}

#[cfg(feature = "reqwest-middleware")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest-middleware")))]
impl HttpClientExt for reqwest_middleware::ClientWithMiddleware {
    fn send<T, U>(
        &self,
        req: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        T: Into<Bytes>,
        U: From<Bytes> + WasmCompatSend,
    {
        let (parts, body) = req.into_parts();
        let req = self
            .request(parts.method, parts.uri.to_string())
            .headers(parts.headers)
            .body(body.into());

        async move {
            let response = req.send().await.map_err(instance_error)?;
            if !response.status().is_success() {
                return Err(Error::InvalidStatusCodeWithMessage(
                    response.status(),
                    response.text().await.unwrap(),
                ));
            }

            let mut res = Response::builder().status(response.status());

            if let Some(hs) = res.headers_mut() {
                *hs = response.headers().clone();
            }

            let body: LazyBody<U> = Box::pin(async {
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|e| Error::Instance(e.into()))?;

                let body = U::from(bytes);
                Ok(body)
            });

            res.body(body).map_err(Error::Protocol)
        }
    }

    fn send_multipart<U>(
        &self,
        req: Request<MultipartForm>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
    where
        U: From<Bytes>,
        U: WasmCompatSend + 'static,
    {
        let (parts, body) = req.into_parts();
        let body = reqwest::multipart::Form::from(body);

        let req = self
            .request(parts.method, parts.uri.to_string())
            .headers(parts.headers)
            .multipart(body);

        async move {
            let response = req.send().await.map_err(instance_error)?;
            if !response.status().is_success() {
                return Err(Error::InvalidStatusCodeWithMessage(
                    response.status(),
                    response.text().await.unwrap(),
                ));
            }

            let mut res = Response::builder().status(response.status());

            if let Some(hs) = res.headers_mut() {
                *hs = response.headers().clone();
            }

            let body: LazyBody<U> = Box::pin(async {
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|e| Error::Instance(e.into()))?;

                let body = U::from(bytes);
                Ok(body)
            });

            res.body(body).map_err(Error::Protocol)
        }
    }

    fn send_streaming<T>(
        &self,
        req: Request<T>,
    ) -> impl Future<Output = Result<StreamingResponse>> + WasmCompatSend
    where
        T: Into<Bytes>,
    {
        let (parts, body) = req.into_parts();

        let req = self
            .request(parts.method, parts.uri.to_string())
            .headers(parts.headers)
            .body(body.into())
            .build()
            .map_err(|x| Error::Instance(x.into()))
            .unwrap();

        let client = self.clone();

        async move {
            let response: reqwest::Response = client.execute(req).await.map_err(instance_error)?;
            if !response.status().is_success() {
                return Err(Error::InvalidStatusCodeWithMessage(
                    response.status(),
                    response.text().await.unwrap(),
                ));
            }

            #[cfg(not(target_family = "wasm"))]
            let mut res = Response::builder()
                .status(response.status())
                .version(response.version());

            #[cfg(target_family = "wasm")]
            let mut res = Response::builder().status(response.status());

            if let Some(hs) = res.headers_mut() {
                *hs = response.headers().clone();
            }

            use futures::StreamExt;

            let mapped_stream: Pin<Box<dyn WasmCompatSendStream<InnerItem = Result<Bytes>>>> =
                Box::pin(
                    response
                        .bytes_stream()
                        .map(|chunk| chunk.map_err(|e| Error::Instance(Box::new(e)))),
                );

            res.body(mapped_stream).map_err(Error::Protocol)
        }
    }
}

/// Test utilities for mocking HTTP clients.
#[cfg(test)]
pub(crate) mod mock {
    use super::*;
    use bytes::Bytes;

    /// A mock HTTP client that returns pre-built SSE bytes from `send_streaming`.
    ///
    /// `send` and `send_multipart` always return `NOT_IMPLEMENTED`.
    #[derive(Clone)]
    pub struct MockStreamingClient {
        pub sse_bytes: Bytes,
    }

    impl HttpClientExt for MockStreamingClient {
        fn send<T, U>(
            &self,
            _req: Request<T>,
        ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
        where
            T: Into<Bytes>,
            T: WasmCompatSend,
            U: From<Bytes>,
            U: WasmCompatSend + 'static,
        {
            std::future::ready(Err(Error::InvalidStatusCode(
                http::StatusCode::NOT_IMPLEMENTED,
            )))
        }

        fn send_multipart<U>(
            &self,
            _req: Request<MultipartForm>,
        ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + WasmCompatSend + 'static
        where
            U: From<Bytes>,
            U: WasmCompatSend + 'static,
        {
            std::future::ready(Err(Error::InvalidStatusCode(
                http::StatusCode::NOT_IMPLEMENTED,
            )))
        }

        fn send_streaming<T>(
            &self,
            _req: Request<T>,
        ) -> impl Future<Output = Result<StreamingResponse>> + WasmCompatSend
        where
            T: Into<Bytes>,
        {
            let sse_bytes = self.sse_bytes.clone();
            async move {
                let byte_stream = futures::stream::iter(vec![Ok::<Bytes, Error>(sse_bytes)]);
                let boxed_stream: BoxedStream = Box::pin(byte_stream);

                Response::builder()
                    .status(http::StatusCode::OK)
                    .header(http::header::CONTENT_TYPE, "text/event-stream")
                    .body(boxed_stream)
                    .map_err(Error::Protocol)
            }
        }
    }
}
