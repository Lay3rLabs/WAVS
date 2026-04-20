//! This module contains clients for the different LLM providers that Rig supports.
//!
//! Currently, the following providers are supported:
//! - Cohere
//! - OpenAI
//! - Perplexity
//! - Anthropic
//! - Google Gemini
//! - xAI
//! - EternalAI
//! - DeepSeek
//! - Azure OpenAI
//! - Mira
//!
//! Each provider has its own module, which contains a `Client` implementation that can
//! be used to initialize completion and embedding models and execute requests to those models.
//!
//! The clients also contain methods to easily create higher level AI constructs such as
//! agents and RAG systems, reducing the need for boilerplate.
//!
//! # Example
//! ```
//! use rig::{providers::openai, agent::AgentBuilder};
//!
//! // Initialize the OpenAI client
//! let openai = openai::Client::new("your-openai-api-key");
//!
//! // Create a model and initialize an agent
//! let gpt_4o = openai.completion_model("gpt-4o");
//!
//! let agent = AgentBuilder::new(gpt_4o)
//!     .preamble("\
//!         You are Gandalf the white and you will be conversing with other \
//!         powerful beings to discuss the fate of Middle Earth.\
//!     ")
//!     .build();
//!
//! // Alternatively, you can initialize an agent directly
//! let agent = openai.agent("gpt-4o")
//!     .preamble("\
//!         You are Gandalf the white and you will be conversing with other \
//!         powerful beings to discuss the fate of Middle Earth.\
//!     ")
//!     .build();
//! ```
//! Note: The example above uses the OpenAI provider client, but the same pattern can
//! be used with the Cohere provider client.
// Anthropic is available on all targets including wasm32-wasip2.
// P7: streaming.rs is gated within anthropic/mod.rs; non-streaming completions work on WASM.
pub mod anthropic;

// All other providers use SSE/streaming which requires non-WASM platform features.
// Gate them out on WASM; only anthropic is needed for WASI agent components.
#[cfg(not(target_family = "wasm"))]
pub mod azure;
#[cfg(not(target_family = "wasm"))]
pub mod cohere;
#[cfg(not(target_family = "wasm"))]
pub mod deepseek;
#[cfg(not(target_family = "wasm"))]
pub mod galadriel;
#[cfg(not(target_family = "wasm"))]
pub mod gemini;
#[cfg(not(target_family = "wasm"))]
pub mod groq;
#[cfg(not(target_family = "wasm"))]
pub mod huggingface;
#[cfg(not(target_family = "wasm"))]
pub mod hyperbolic;
#[cfg(not(target_family = "wasm"))]
pub mod llamafile;
#[cfg(not(target_family = "wasm"))]
pub mod mira;
#[cfg(not(target_family = "wasm"))]
pub mod mistral;
#[cfg(not(target_family = "wasm"))]
pub mod moonshot;
#[cfg(not(target_family = "wasm"))]
pub mod ollama;
#[cfg(not(target_family = "wasm"))]
pub mod openai;
#[cfg(not(target_family = "wasm"))]
pub mod openrouter;
#[cfg(not(target_family = "wasm"))]
pub mod perplexity;
#[cfg(not(target_family = "wasm"))]
pub mod together;
#[cfg(not(target_family = "wasm"))]
pub mod voyageai;
#[cfg(not(target_family = "wasm"))]
pub mod xai;
