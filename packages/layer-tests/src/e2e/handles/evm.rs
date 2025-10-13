use std::process::{Command, Stdio};

use utils::{config::EvmChainConfig, context::AppContext};

use crate::e2e::config::Configs;

pub struct EvmInstance {
    _anvil: LameAnvilInstance,
    _chain_config: EvmChainConfig,
}

impl EvmInstance {
    pub fn spawn(_ctx: AppContext, _configs: &Configs, chain_config: EvmChainConfig) -> Self {
        let port = chain_config
            .http_endpoint
            .as_ref()
            .unwrap_or_else(|| chain_config.ws_endpoints.iter().next().unwrap())
            .split(':')
            .next_back()
            .unwrap()
            .parse::<u16>()
            .unwrap();

        tracing::info!(
            "Starting EVM chain: {} on port {}",
            chain_config.chain_id,
            port
        );

        // Something is broken with Alloy's anvil thing... let's use our own
        let anvil = LameAnvilInstanceBuilder {
            port,
            chain_id: chain_config.chain_id.to_string(),
        }
        .spawn();

        Self {
            _anvil: anvil,
            _chain_config: chain_config,
        }
    }
}

// TODO: keep an eye on this
// Latest Alloy breaks things... not sure why.
// but their code doesn't really do anything more than what we have here, is just a bit more opinionated and parses the output
// and this way we have more control, so probably leave it as it is, at least until it breaks again :P

struct LameAnvilInstanceBuilder {
    pub port: u16,
    pub chain_id: String,
}

impl LameAnvilInstanceBuilder {
    pub fn spawn(self) -> LameAnvilInstance {
        let args = vec![
            "-p".to_string(),
            self.port.to_string(),
            "--chain-id".to_string(),
            self.chain_id,
            "--block-time".to_string(),
            "1".to_string(),
        ];

        let child = Command::new("anvil")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();

        LameAnvilInstance { child }
    }
}

struct LameAnvilInstance {
    child: std::process::Child,
}

impl Drop for LameAnvilInstance {
    fn drop(&mut self) {
        if let Err(err) = self.child.kill() {
            tracing::error!("Failed to kill anvil: {}", err);
        }
    }
}

impl Drop for EvmInstance {
    fn drop(&mut self) {
        tracing::info!("Stopping EVM chain: {}", self._chain_config.chain_id);
    }
}
