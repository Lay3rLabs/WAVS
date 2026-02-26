mod chain_ops;
mod client;
mod scaffold;
mod server;

use clap::Parser;
use rmcp::{serve_server, transport::io::stdio};

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
    #[arg(long, env = "WAVS_CHAIN_WRITE_CREDENTIAL")]
    chain_write_credential: Option<String>,

    /// BIP39 mnemonic for the WAVS signing key.
    /// Required (alongside --chain-write-credential) for: wavs_register_operator.
    #[arg(long, env = "WAVS_SIGNING_MNEMONIC")]
    signing_mnemonic: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

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
