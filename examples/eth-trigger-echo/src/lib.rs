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
    // TODO experiment with generate endpoint
    let mut req = Request::post("http://localhost:11434/api/chat")?;

    req.json(&serde_json::json!({
        // https://github.com/ollama/ollama/blob/main/docs/api.md
        "model": "llama3.1",
        "messages": [{
            "role": "system",
            "content": "You are an Avante Garde philosopher, Gilles Deleuze."
        }, {
            "role": "user",
            "content": prompt
        }],

        // TODO: figure out how to use this, prompt should mention structured output
        // "format": "json",

        // Structured output control (haven't figured out how to use this yet)
        // "format": {
        //     "type": "object",
        //     "properties": {
        //         "name": { "type": "string" },
        //         "description": { "type": "string" },
        //     },
        //     "required": ["name", "description"]
        // },

        // Core options for deterministic output
        "options": {
            // Sampling strategy (deterministic focus)
            "temperature": 0.0,        // [0.0-2.0] 0.0 for most deterministic
            "top_k": 1,               // [1-100] 1 for strict selection
            "top_p": 0.1,             // [0.0-1.0] 0.1 for narrow sampling
            "min_p": 0.0,             // [0.0-1.0] Alternative to top_p (disabled)

            // Repetition control
            // "repeat_last_n": 64,      // [-1, 0-N] tokens to look back (-1 = num_ctx)
            // "repeat_penalty": 1.2,     // [0.0-2.0] Higher = less repetition

            // Mirostat sampling (alternative to temperature)
            // "mirostat": 0,         // [0-2] 0=disabled, 1=v1, 2=v2
            // "mirostat_tau": 5.0,   // [0.0-10.0] Lower = more focused
            // "mirostat_eta": 0.1,   // [0.0-1.0] Learning rate

            // Context and length control
            "num_ctx": 4096,          // [512-8192] Context window size
            "num_predict": 500,       // [-1, 1-N] Max tokens to generate (-1 = infinite)

            // Stop sequences (model-specific)
            // "stop": [
            //     "\n\n",              // Common stop
            //     "###",               // Common stop
            //     "<|im_start|>",      // Chat format
            //     "<|im_end|>",        // Chat format
            //     "```",               // Code blocks
            //     "USER:",             // Chat roles
            //     "ASSISTANT:"         // Chat roles
            // ],

            // Deterministic generation
            "seed": 42,              // Fixed seed for reproducibility

            // System resource management
            // "num_thread": 8,         // CPU threads to use
            // "num_gpu": 1,            // Number of GPUs to use
            // "num_batch": 2,       // Batch size for prompt processing
            // "num_keep": 5,        // Number of tokens to keep from initial prompt

            // Memory management
            // "low_vram": false,    // Optimize for low VRAM GPUs
            // "main_gpu": 0,        // Main GPU index
            // "numa": false,        // NUMA acceleration
            // "use_mmap": true,     // Memory-mapped I/O
            // "use_mlock": false,   // Lock memory
        },

        // API behavior
        "stream": false,             // No streaming for consistent response
        // "keep_alive": "5m",         // Model keep-alive duration

        // Raw mode (bypass template system)
        // "raw": true,             // Enable if using custom prompt format
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
