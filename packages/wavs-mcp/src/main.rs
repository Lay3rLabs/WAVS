mod chain_ops;
mod client;
mod scaffold;
mod server;

use clap::Parser;
use rmcp::{serve_server, transport::io::stdio};
use utils::config::ConfigFilePath;

#[derive(Parser)]
#[command(
    name = "wavs-mcp",
    about = "WAVS MCP Server — exposes WAVS platform operations via Model Context Protocol"
)]
struct Args {
    /// URL of the WAVS node HTTP API
    #[arg(long, env = "WAVS_URL", default_value = "http://localhost:8000")]
    wavs_url: String,

    /// Bearer token for write operations (deploy, pause, resume)
    #[arg(long, env = "WAVS_TOKEN")]
    token: Option<String>,

    /// Credential (private key `0x…` or BIP39 mnemonic) for on-chain management transactions.
    /// Required for: wavs_deploy_service_manager, wavs_deploy_poa_service_manager,
    /// wavs_register_operator, wavs_set_service_uri.
    /// Falls back to `chain_write_credential` in the [wavs] section of wavs.toml.
    #[arg(long, env = "WAVS_CHAIN_WRITE_CREDENTIAL")]
    chain_write_credential: Option<String>,

    /// BIP39 mnemonic for the WAVS signing key.
    /// Required (alongside --chain-write-credential) for: wavs_register_operator.
    /// Falls back to `signing_mnemonic` in the [wavs] section of wavs.toml.
    #[arg(long, env = "WAVS_SIGNING_MNEMONIC")]
    signing_mnemonic: Option<String>,
}

/// Read a string field from the [wavs] section of wavs.toml.
/// Searches all candidate paths in order until the field is found.
/// This way a local wavs.toml that lacks a field gracefully falls through to
/// the global ~/.wavs/wavs.toml (written by setup_claude_mcp.py).
fn read_wavs_toml_field(field: &str) -> Option<String> {
    for path in ConfigFilePath::new("wavs.toml", None).into_possible() {
        if !path.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else { continue };
        let Ok(doc) = content.parse::<toml::Table>() else { continue };
        if let Some(value) = doc.get("wavs").and_then(|v| v.get(field)).and_then(|v| v.as_str()) {
            return Some(value.to_string());
        }
    }
    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mut args = Args::parse();

    // Fall back to wavs.toml [wavs] section for credentials not set via CLI/env.
    if args.chain_write_credential.is_none() {
        args.chain_write_credential = read_wavs_toml_field("chain_write_credential");
    }
    if args.signing_mnemonic.is_none() {
        args.signing_mnemonic = read_wavs_toml_field("signing_mnemonic");
    }

    tracing::info!("Starting WAVS MCP server, connecting to {}", args.wavs_url);

    let server = server::WavsMcpServer::new(
        args.wavs_url,
        args.token,
        args.chain_write_credential,
        args.signing_mnemonic,
    );

    serve_server(server, stdio())
        .await
        .map_err(|e| anyhow::anyhow!("MCP server error: {e}"))?
        .waiting()
        .await
        .map_err(|e| anyhow::anyhow!("MCP server task error: {e}"))?;

    Ok(())
}
