//! HTTP helpers to make requests. Will eventually be deprecated by improvements to wstd, reqwest, etc.
use http::Request;
use serde::{de::DeserializeOwned, Serialize};
use wstd::{
    http::{body::BoundedBody, Body, Client, IntoBody},
    io::{empty, AsyncRead, Empty},
};

/// Helper to just get a url
pub fn http_request_get(url: &str) -> anyhow::Result<Request<Empty>> {
    Request::get(url).body(empty()).map_err(|e| e.into())
}

/// Helper to post a url + json
pub fn http_request_post_json(
    url: &str,
    body: impl Serialize,
) -> anyhow::Result<Request<BoundedBody<Vec<u8>>>> {
    let body = serde_json::to_vec(&body)?;

    Ok(Request::post(url)
        .header("content-type", "application/json")
        .body(body.into_body())?)
}

/// Helper to post a url + form data (as www-form-urlencoded)
pub fn http_request_post_form(
    url: &str,
    form_data: impl IntoIterator<Item = (String, String)>,
) -> anyhow::Result<Request<BoundedBody<Vec<u8>>>> {
    let mut body = String::new();
    for (key, value) in form_data {
        if !body.is_empty() {
            body += "&";
        }
        body += &format!("{}={}\n", key, value);
    }

    Ok(Request::post(url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body.into_body())?)
}

/// Fetch a request (typically constructed from one of the http_request_* helpers)
/// Returns raw bytes
pub async fn fetch_bytes(request: Request<impl Body>) -> anyhow::Result<Vec<u8>> {
    let mut response = Client::new().send(request).await?;

    let body = response.body_mut();
    let mut body_buf = Vec::new();
    body.read_to_end(&mut body_buf).await?;

    Ok(body_buf)
}

/// Fetch a request (typically constructed from one of the http_request_* helpers)
/// Deserializes the response into a JSON type
pub async fn fetch_json<T: DeserializeOwned>(request: Request<impl Body>) -> anyhow::Result<T> {
    let bytes = fetch_bytes(request).await?;

    Ok(serde_json::from_slice(&bytes)?)
}

/// Fetch a request (typically constructed from one of the http_request_* helpers)
/// Deserializes the response into a UTF-8 string
pub async fn fetch_string(request: Request<impl Body>) -> anyhow::Result<String> {
    let bytes = fetch_bytes(request).await?;

    Ok(String::from_utf8(bytes)?)
}
