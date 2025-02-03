use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use layer_wasi::{
    bindings::world::{Guest, TriggerAction},
    export_layer_trigger_world,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use wstd::{
    http::{Client, Request},
    io::{empty, AsyncRead},
    runtime::block_on,
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use url::Url;

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Vec<u8>, String> {
        block_on(async move {
            let (trigger_id, req) =
                decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;
            let req: PermissionsInput = serde_json::from_slice(&req).map_err(|e| e.to_string())?;
            let resp = inner_run_task(req).await.map_err(|e| e.to_string())?;
            let resp = serde_json::to_vec(&resp).map_err(|e| e.to_string())?;
            Ok(encode_trigger_output(trigger_id, resp))
        })
    }
}

async fn inner_run_task(input: PermissionsInput) -> Result<Response> {
    const DIRECTORY_NAME: &'static str = "./responses";

    let responses_path = Path::new(DIRECTORY_NAME);
    if !responses_path.exists() {
        fs::create_dir_all(DIRECTORY_NAME)?;
    }

    let response_path = responses_path.join(format!("{}.txt", input.timestamp));
    let mut response_file = fs::File::create(&response_path)?;

    let contents = get_url(Url::parse(&input.url)?).await?;

    response_file.write_all(contents.as_bytes())?;

    let responses_count = fs::read_dir(responses_path)?.count();

    Ok(Response {
        filename: response_path.to_path_buf(),
        contents,
        filecount: responses_count,
    })
}

async fn get_url(url: Url) -> Result<String> {
    let request = Request::get(url.to_string())
        .body(empty())
        .map_err(|e| anyhow!("{e:?}"))?;
    let mut response = Client::new()
        .send(request)
        .await
        .map_err(|e| anyhow!("{e:?}"))?;
    let body = response.body_mut();
    let mut body_buf = Vec::new();
    body.read_to_end(&mut body_buf)
        .await
        .map_err(|e| anyhow!("{e:?}"))?;
    Ok(serde_json::to_string(&body_buf).map_err(|e| anyhow!("{e:?}"))?)
}

#[derive(Deserialize, Serialize)]
struct PermissionsInput {
    pub url: String,
    pub timestamp: u64,
}

#[derive(Deserialize, Serialize, Debug)]
struct Response {
    pub filename: PathBuf,
    pub contents: String,
    pub filecount: usize,
}

export_layer_trigger_world!(Component);
