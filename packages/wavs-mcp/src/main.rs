mod chain_ops;
mod client;
mod exec;
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
    /// Falls back to `mcp_chain_credential` in the [wavs] section of ~/.wavs/wavs.toml.
    #[arg(long, env = "WAVS_MCP_CHAIN_CREDENTIAL")]
    mcp_chain_credential: Option<String>,

    /// BIP39 mnemonic for the WAVS signing key.
    /// Required (alongside --mcp-chain-credential) for: wavs_register_operator.
    /// Falls back to `signing_mnemonic` in the [wavs] section of ~/.wavs/wavs.toml.
    #[arg(long, env = "WAVS_SIGNING_MNEMONIC")]
    signing_mnemonic: Option<String>,

    /// Enable execution tools (wavs_exec_*). When disabled, only management tools are available.
    /// This is a safety gate -- execution tools can invoke component logic and (for Tier 3)
    /// submit on-chain transactions.
    #[arg(long, env = "WAVS_EXEC_ENABLED", default_value = "false")]
    exec_enabled: bool,
}

/// Read a credential field from the [wavs] section of wavs.toml, searching only
/// user-home paths (~/.wavs/wavs.toml, ~/.config/wavs/wavs.toml, etc.).
///
/// Paths under `WAVS_HOME` env var or the process CWD are intentionally skipped.
/// Project-local wavs.toml files may live inside git repositories and risk accidental
/// commit of secrets. Only user-home and system paths are considered safe.
fn read_credential_toml_field(field: &str) -> Option<String> {
    let cwd = std::env::current_dir().ok();
    let wavs_home_env = std::env::var("WAVS_HOME")
        .ok()
        .map(std::path::PathBuf::from);

    for path in ConfigFilePath::new("wavs.toml", None).into_possible() {
        // Skip CWD-relative paths (project-local files)
        if let Some(ref cwd) = cwd {
            if path.starts_with(cwd) {
                continue;
            }
        }
        // Skip WAVS_HOME paths (also potentially project-local)
        if let Some(ref home) = wavs_home_env {
            if path.starts_with(home) {
                continue;
            }
        }

        if !path.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(doc) = content.parse::<toml::Table>() else {
            continue;
        };
        if let Some(value) = doc
            .get("wavs")
            .and_then(|v| v.get(field))
            .and_then(|v| v.as_str())
        {
            tracing::info!("Loaded '{}' from {}.", field, path.display());
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

    // Fall back to user-home wavs.toml [wavs] section for credentials not set via CLI/env.
    // CWD and WAVS_HOME paths are excluded to prevent loading secrets from project-local
    // files that may be committed to a git repository.
    if args.mcp_chain_credential.is_none() {
        args.mcp_chain_credential = read_credential_toml_field("mcp_chain_credential");
    }
    if args.signing_mnemonic.is_none() {
        args.signing_mnemonic = read_credential_toml_field("signing_mnemonic");
    }

    tracing::info!("Starting WAVS MCP server, connecting to {}", args.wavs_url);

    let server = server::WavsMcpServer::new(
        args.wavs_url,
        args.token,
        args.mcp_chain_credential,
        args.signing_mnemonic,
        args.exec_enabled,
    );

    serve_server(server, stdio())
        .await
        .map_err(|e| anyhow::anyhow!("MCP server error: {e}"))?
        .waiting()
        .await
        .map_err(|e| anyhow::anyhow!("MCP server task error: {e}"))?;

    Ok(())
}
