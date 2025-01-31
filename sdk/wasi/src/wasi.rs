// originally from https://raw.githubusercontent.com/Lay3rLabs/avs-toolkit/refs/heads/main/packages/layer-wasi/src/lib.rs

#![allow(async_fn_in_trait)]

use serde::{de::DeserializeOwned, Serialize};
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
