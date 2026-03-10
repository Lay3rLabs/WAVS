//! HTTP helpers to make requests. Will eventually be deprecated by improvements to wstd, reqwest, etc.
use serde::{de::DeserializeOwned, Serialize};
use wstd::http::{Body, Client, Request};

/// Helper to just get a url
pub fn http_request_get(url: &str) -> anyhow::Result<Request<Body>> {
    Ok(Request::get(url).body(Body::empty())?)
}

/// Helper to post a url + json
pub fn http_request_post_json(url: &str, body: impl Serialize) -> anyhow::Result<Request<Body>> {
    let body = serde_json::to_vec(&body)?;

    Ok(Request::post(url)
        .header("content-type", "application/json")
        .body(Body::from(body))?)
}

/// Helper to post a url + form data (as www-form-urlencoded)
pub fn http_request_post_form(
    url: &str,
    form_data: impl IntoIterator<Item = (String, String)>,
) -> anyhow::Result<Request<Body>> {
    let mut body = String::new();
    for (key, value) in form_data {
        if !body.is_empty() {
            body += "&";
        }
        body += &format!("{key}={value}\n");
    }

    Ok(Request::post(url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(body))?)
}

/// Fetch a request (typically constructed from one of the http_request_* helpers)
/// Returns raw bytes
pub async fn fetch_bytes(request: Request<impl Into<Body>>) -> anyhow::Result<Vec<u8>> {
    let mut response = Client::new().send(request).await?;

    let body_bytes = response.body_mut().contents().await?;

    Ok(body_bytes.to_vec())
}

/// Fetch a request (typically constructed from one of the http_request_* helpers)
/// Deserializes the response into a JSON type
pub async fn fetch_json<T: DeserializeOwned>(
    request: Request<impl Into<Body>>,
) -> anyhow::Result<T> {
    let bytes = fetch_bytes(request).await?;

    Ok(serde_json::from_slice(&bytes)?)
}

/// Fetch a request (typically constructed from one of the http_request_* helpers)
/// Deserializes the response into a UTF-8 string
pub async fn fetch_string(request: Request<impl Into<Body>>) -> anyhow::Result<String> {
    let bytes = fetch_bytes(request).await?;

    Ok(String::from_utf8(bytes)?)
}
