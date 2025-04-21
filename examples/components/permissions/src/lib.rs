use example_helpers::bindings::compat::LogLevel;
use example_helpers::bindings::world::WasmResponse;
use example_helpers::trigger::{decode_trigger_event, encode_trigger_output};
use example_helpers::{
    bindings::world::{host, Guest, TriggerAction},
    export_layer_trigger_world,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use wavs_wasi_utils::http::{fetch_json, fetch_string, http_request_get, http_request_post_json};
use wstd::runtime::block_on;

use anyhow::Result;
use serde::{Deserialize, Serialize};

struct Component;

impl Guest for Component {
    fn run(trigger_action: TriggerAction) -> std::result::Result<Option<WasmResponse>, String> {
        block_on(async move {
            let (trigger_id, req) =
                decode_trigger_event(trigger_action.data).map_err(|e| e.to_string())?;

            println!("(permissions println!) trigger id: {}", trigger_id);
            eprintln!("(permissions eprintln!) trigger id: {}", trigger_id);
            host::log(
                LogLevel::Info,
                &format!("(permissions host log) trigger id: {}", trigger_id),
            );

            let req: PermissionsInput = serde_json::from_slice(&req).map_err(|e| e.to_string())?;
            let resp = inner_run_task(req).await.map_err(|e| e.to_string())?;
            let resp = serde_json::to_vec(&resp).map_err(|e| e.to_string())?;
            Ok(Some(encode_trigger_output(trigger_id, resp)))
        })
    }
}

async fn inner_run_task(input: PermissionsInput) -> Result<Response> {
    const DIRECTORY_NAME: &str = "./responses";

    let responses_path = Path::new(DIRECTORY_NAME);
    if !responses_path.exists() {
        fs::create_dir_all(DIRECTORY_NAME)?;
    }

    let response_path = responses_path.join(format!("{}.txt", input.timestamp));
    let mut response_file = fs::File::create(&response_path)?;

    let get_response = fetch_string(http_request_get(&input.get_url)?).await?;

    #[derive(Deserialize, Debug)]
    struct PostResponse {
        json: (String, String),
    }

    let post_response: PostResponse =
        fetch_json(http_request_post_json(&input.post_url, &input.post_data)?).await?;

    if post_response.json != input.post_data {
        return Err(anyhow::anyhow!(
            "The post data is not the same as the one sent"
        ));
    }

    let contents = format!(
        "GET RESPONSE: {}\n\nPOST RESPONSE: {:?}",
        get_response, post_response
    );

    response_file.write_all(contents.as_bytes())?;

    let responses_count = fs::read_dir(responses_path)?.count();

    Ok(Response {
        filename: response_path.to_path_buf(),
        contents,
        filecount: responses_count,
    })
}

#[derive(Deserialize, Serialize)]
struct PermissionsInput {
    pub get_url: String,
    pub post_url: String,
    pub post_data: (String, String),
    pub timestamp: u64,
}

#[derive(Deserialize, Serialize, Debug)]
struct Response {
    pub filename: PathBuf,
    pub contents: String,
    pub filecount: usize,
}

export_layer_trigger_world!(Component);
