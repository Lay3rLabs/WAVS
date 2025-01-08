#[allow(warnings)]
mod bindings;

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use bindings::{Contract, Guest};
use serde::{Deserialize, Serialize};
use url::Url;
use wasi::{
    http::{
        outgoing_handler,
        types::{Headers, InputStream, OutgoingRequest, Scheme},
    },
    io::streams::StreamError,
};

struct Component;

impl Guest for Component {
    fn run(_contract: Contract, input: Vec<u8>) -> Result<Vec<u8>, String> {
        match inner_run_task(serde_json::from_slice(&input).map_err(|x| x.to_string())?) {
            Ok(response) => serde_json::to_vec(&response).map_err(|x| x.to_string()),
            Err(e) => Err(e.to_string()),
        }
    }
}

fn inner_run_task(input: PermissionsInput) -> Result<Response> {
    const DIRECTORY_NAME: &'static str = "./responses";

    let responses_path = Path::new(DIRECTORY_NAME);
    if !responses_path.exists() {
        fs::create_dir_all(DIRECTORY_NAME)?;
    }

    let response_path = responses_path.join(format!("{}.txt", input.timestamp));
    let mut response_file = fs::File::create(&response_path)?;

    let contents = get_url(Url::parse(&input.url)?)?;

    response_file.write_all(contents.as_bytes())?;

    let responses_count = fs::read_dir(responses_path)?.count();

    Ok(Response {
        filename: response_path.to_path_buf(),
        contents,
        filecount: responses_count,
    })
}

fn get_url(url: Url) -> Result<String> {
    let req = OutgoingRequest::new(Headers::new());
    req.set_scheme(Some(&Scheme::Https))
        .map_err(|e| anyhow!("{e:?}"))?;

    req.set_authority(Some(url.authority()))
        .map_err(|e| anyhow!("{e:?}"))?;
    req.set_path_with_query(Some(url.path()))
        .map_err(|e| anyhow!("{e:?}"))?;

    let fut = outgoing_handler::handle(req, None)?;
    let subscription = fut.subscribe();

    subscription.block();

    let res = fut
        .get()
        .context("None in response")?
        .map_err(|e| anyhow!("{e:?}"))??;

    if !(200..=299).contains(&res.status()) {
        return Err(anyhow::anyhow!("unexpected status code: {}", res.status()));
    }

    let body = res.consume().map_err(|e| anyhow!("{e:?}"))?;
    let stream = body.stream().map_err(|e| anyhow!("{e:?}"))?;

    let contents = read_stream_to_string(&stream)?;

    Ok(contents)
}

fn read_stream_to_string(stream: &InputStream) -> Result<String> {
    let mut buffer = Vec::new();
    let read_len = 1024;

    loop {
        match stream.blocking_read(read_len) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break;
                }
                buffer.extend_from_slice(&chunk);
            }
            Err(e) => match e {
                StreamError::LastOperationFailed(error) => {
                    return Err(anyhow!("{error:?}"));
                }
                StreamError::Closed => {
                    break;
                }
            },
        }
    }

    let result_string = String::from_utf8(buffer)?;
    Ok(result_string)
}

#[derive(Deserialize, Serialize)]
struct PermissionsInput {
    pub url: String,
    pub timestamp: u64,
}

#[derive(Deserialize, Serialize)]
struct Response {
    pub filename: PathBuf,
    pub contents: String,
    pub filecount: usize,
}

bindings::export!(Component with_types_in bindings);
