// originally from https://raw.githubusercontent.com/Lay3rLabs/avs-toolkit/refs/heads/main/packages/layer-wasi/src/lib.rs

#![allow(async_fn_in_trait)]

use serde::{de::DeserializeOwned, Serialize};
use std::cmp::min;
pub use url::Url;
pub use wasi::http::types::Method;
pub use wstd::runtime::{block_on, Reactor};

/// The error type.
pub type Error = String;

/// The result type.
pub type Result<T> = std::result::Result<T, Error>;

/// An HTTP request.
#[derive(Debug)]
pub struct Request {
    pub method: Method,
    pub url: Url,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    /// Construct request.
    pub fn new(method: Method, url: &str) -> Result<Self> {
        Ok(Self {
            method,
            url: Url::parse(url).map_err(|e| e.to_string())?,
            headers: vec![],
            body: vec![],
        })
    }

    /// Construct GET request.
    pub fn get(url: &str) -> Result<Self> {
        Request::new(Method::Get, url)
    }

    /// Construct POST request.
    pub fn post(url: &str) -> Result<Self> {
        Request::new(Method::Post, url)
    }

    /// Construct PUT request.
    pub fn put(url: &str) -> Result<Self> {
        Request::new(Method::Put, url)
    }

    /// Construct PATCH request.
    pub fn patch(url: &str) -> Result<Self> {
        Request::new(Method::Patch, url)
    }

    /// Construct DELETE request.
    pub fn delete(url: &str) -> Result<Self> {
        Request::new(Method::Delete, url)
    }

    /// Set JSON body.
    pub fn json<T: Serialize + ?Sized>(&mut self, json: &T) -> Result<&mut Self> {
        self.body = serde_json::to_vec(json).map_err(|e| e.to_string())?;

        if !self
            .headers
            .iter()
            .any(|(k, _)| &k.to_lowercase() == "content-type")
        {
            self.headers
                .push(("content-type".to_string(), "application/json".to_string()));
        }

        Ok(self)
    }
}

/// An HTTP response.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// Get JSON body.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.body).map_err(|e| e.to_string())
    }
}
/// Trait adding WASI methods to the `wstd::runtime::Reactor`.
pub trait WasiPollable {
    async fn read_all(
        &self,
        stream: wasi::io::streams::InputStream,
        size: Option<usize>,
    ) -> Result<Vec<u8>>;
    async fn write_all(&self, stream: wasi::io::streams::OutputStream, bytes: &[u8]) -> Result<()>;
    async fn send(&self, req: Request) -> Result<Response>;
}

impl WasiPollable for wstd::runtime::Reactor {
    /// Read `wasi:io` `input-stream` into memory.
    async fn read_all(
        &self,
        stream: wasi::io::streams::InputStream,
        size: Option<usize>,
    ) -> Result<Vec<u8>> {
        let mut buf = if let Some(size) = size {
            Vec::with_capacity(size)
        } else {
            Vec::new()
        };

        loop {
            // wait for stream to be available for reading
            self.wait_for(stream.subscribe()).await;

            // read bytes
            let mut bytes = match stream.read(4096) {
                Ok(bytes) => bytes,
                Err(wasi::io::streams::StreamError::Closed) => {
                    return Ok(buf);
                }
                Err(wasi::io::streams::StreamError::LastOperationFailed(err)) => {
                    return Err(format!(
                        "failed to read from stream: {}",
                        err.to_debug_string()
                    ))
                }
            };
            buf.append(&mut bytes);
        }
    }

    /// Write `wasi:io` `output-stream` from memory.
    async fn write_all(
        &self,
        stream: wasi::io::streams::OutputStream,
        mut bytes: &[u8],
    ) -> Result<()> {
        let err = "failed to write to stream";

        while !bytes.is_empty() {
            // wait for stream to be available to write
            self.wait_for(stream.subscribe()).await;

            // how many bytes the stream accept next
            let n = stream.check_write().map_err(|_| err)? as usize;
            let stop = min(n, bytes.len());

            // write and flush and advance slice
            stream.write(&bytes[..stop]).map_err(|_| err)?;
            stream.flush().map_err(|_| err)?;

            if stop == bytes.len() {
                // wait for stream to finish flush
                self.wait_for(stream.subscribe()).await;
                break;
            } else {
                bytes = &bytes[stop..];
            }
        }

        Ok(())
    }

    /// Send the HTTP request.
    async fn send(&self, req: Request) -> Result<Response> {
        let wasi_headers = wasi::http::types::Fields::from_list(
            &req.headers
                .into_iter()
                .map(|(k, v)| (k, v.into_bytes()))
                .collect::<Vec<(String, Vec<u8>)>>(),
        )
        .or(Err("invalid header".to_string()))?;

        let wasi_req = wasi::http::types::OutgoingRequest::new(wasi_headers);

        // set the HTTP method
        wasi_req
            .set_method(&req.method)
            .or(Err("invalid method".to_string()))?;

        // Set the url scheme
        use wasi::http::types::Scheme;
        let scheme = match req.url.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            other => Scheme::Other(other.to_owned()),
        };
        wasi_req
            .set_scheme(Some(&scheme))
            .or(Err("invalid url scheme".to_string()))?;

        // Set the url path + query string
        let path = match req.url.query() {
            Some(query) => format!("{}?{query}", req.url.path()),
            None => req.url.path().to_owned(),
        };
        wasi_req
            .set_path_with_query(Some(&path))
            .or(Err("invalid url path".to_string()))?;
        wasi_req
            .set_authority(Some(req.url.authority()))
            .or(Err("invalid url authority".to_string()))?;

        let wasi_body = wasi_req.body().unwrap();
        let body_stream = wasi_body.write().unwrap();

        // start sending the request
        let res = wasi::http::outgoing_handler::handle(wasi_req, None)
            .or(Err("failed to send request".to_string()))?;

        // send the request body
        self.write_all(body_stream, &req.body).await?;

        // finish sending the request body with no trailers
        wasi::http::types::OutgoingBody::finish(wasi_body, None).unwrap();

        // wait for the response
        self.wait_for(res.subscribe()).await;

        let res = res
            .get()
            .unwrap()
            .unwrap()
            .map_err(|err| format!("response error: {err}"))?;

        let res_status = res.status();
        let mut content_length = None;
        let res_headers = res
            .headers()
            .entries()
            .into_iter()
            .map(|(k, v)| {
                if k.to_lowercase() == "content-length" {
                    content_length = std::str::from_utf8(&v)
                        .ok()
                        .and_then(|s| s.parse::<usize>().ok());
                }
                let v = std::string::String::from_utf8(v)
                    .or(Err(format!("invalid response header value for `{k}`")))?;
                Ok((k, v))
            })
            .collect::<Result<Vec<(String, String)>>>()?;

        // read response body
        let res_body = res.consume().unwrap();
        let res_body_stream = res_body.stream().unwrap();

        Ok(Response {
            status: res_status,
            headers: res_headers,
            body: self.read_all(res_body_stream, content_length).await?,
        })
    }
}
