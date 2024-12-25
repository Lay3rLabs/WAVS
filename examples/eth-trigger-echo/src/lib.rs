#[allow(warnings)]
mod bindings;
use anyhow::Result;
use base64;
use bindings::Guest;
use layer_wasi::{block_on, Reactor, Request, WasiPollable};
use serde::{Deserialize, Serialize};

// NFT Metadata structure
#[derive(Serialize)]
struct NFTMetadata {
    name: String,
    description: String,
    image: String,
    attributes: Vec<Attribute>,
}

#[derive(Serialize)]
struct Attribute {
    trait_type: String,
    value: String,
}

// Ollama response structures
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum OllamaChatResponse {
    Success(OllamaChatSuccessResponse),
    Error { error: String },
}

#[derive(Deserialize, Debug)]
struct OllamaChatSuccessResponse {
    message: OllamaChatMessage,
}

#[derive(Deserialize, Debug)]
struct OllamaChatMessage {
    content: String,
}

struct Component;

impl Guest for Component {
    fn process_eth_trigger(input: Vec<u8>) -> std::result::Result<Vec<u8>, String> {
        let prompt =
            String::from_utf8(input.clone()).map_err(|e| format!("Invalid UTF-8: {}", e))?;

        block_on(|reactor| async move {
            // Query Ollama
            let response = query_ollama(&reactor, &prompt).await?;

            // Create NFT metadata
            let metadata = NFTMetadata {
                name: "AI Generated NFT".to_string(),
                description: response,
                image: "ipfs://placeholder".to_string(),
                attributes: vec![Attribute {
                    trait_type: "Prompt".to_string(),
                    value: prompt,
                }],
            };

            // Serialize to JSON and convert to data URI
            let json = serde_json::to_string(&metadata)
                .map_err(|e| format!("JSON serialization error: {}", e))?;
            let data_uri = format!("data:application/json;base64,{}", base64::encode(json));

            Ok(data_uri.into_bytes())
        })
    }
}

async fn query_ollama(reactor: &Reactor, prompt: &str) -> Result<String, String> {
    let mut req = Request::post("http://localhost:11434/api/chat")?;

    // TODO: Make this more of a flushed out configuration with all the options
    req.json(&serde_json::json!({
        "model": "llama3.1",
        "messages": [{
            "role": "user",
            "content": prompt
        }],
        "stream": false
    }))?;

    let res = reactor.send(req).await?;

    if res.status != 200 {
        return Err(format!("Ollama API error: status {}", res.status));
    }

    match res.json::<OllamaChatResponse>() {
        Ok(OllamaChatResponse::Success(success)) => Ok(success.message.content),
        Ok(OllamaChatResponse::Error { error }) => Err(error),
        Err(e) => Err(format!("Failed to parse response: {}", e)),
    }
}

bindings::export!(Component with_types_in bindings);
