use std::process::{Command, Stdio};

use layer_climb::prelude::*;
use utils::config::CosmosChainConfig;
use wavs::AppContext;

use crate::e2e::config::Configs;

/// A handle that represents a running Docker container. When dropped, it will attempt
/// to kill (and remove) the container automatically.
pub struct CosmosInstance {
    pub chain_config: layer_climb::prelude::ChainConfig,
}

impl CosmosInstance {
    pub fn setup(
        ctx: AppContext,
        configs: &Configs,
        chain_config: CosmosChainConfig,
    ) -> std::io::Result<Self> {
        tracing::info!("Setting up Cosmos chain: {}", chain_config.chain_id);
        let mnemonic = configs.cli.cosmos_mnemonic.as_ref().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Missing 'cosmos_mnemonic' in CLI config.",
            )
        })?;

        let chain_config: layer_climb::prelude::ChainConfig = chain_config.clone().into();
        let signer = layer_climb::prelude::KeySigner::new_mnemonic_str(mnemonic, None).unwrap();
        let addr: layer_climb::prelude::Address = ctx.rt.block_on(async {
            chain_config
                .address_from_pub_key(&signer.public_key().await.unwrap())
                .unwrap()
        });

        let _self = Self { chain_config };
        _self.clean();

        let name = _self.name();

        let _output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--name",
                &name,
                "--mount",
                &format!("type=volume,source={}_data,target=/root", &name),
                "--env",
                &format!("CHAIN_ID={}", _self.chain_config.chain_id),
                "--env",
                &format!("FEE={}", _self.chain_config.gas_denom),
                "cosmwasm/wasmd:latest",
                "/opt/setup_wasmd.sh",
                &addr.to_string(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        let _output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--name",
                &name,
                "--mount",
                &format!("type=volume,source={}_data,target=/root", &name),
                "cosmwasm/wasmd:latest",
                "sed",
                "-E",
                "-i",
                "/timeout_(propose|prevote|precommit|commit)/s/[0-9]+m?s/200ms/",
                "/root/.wasmd/config/config.toml",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        Ok(_self)
    }

    pub fn name(&self) -> String {
        format!("layer-tests-cosmos-{}", self.chain_config.chain_id)
    }

    pub fn run(self) -> std::io::Result<Self> {
        tracing::info!("Starting Cosmos chain: {}", self.chain_config.chain_id);
        let rpc_port = self
            .chain_config
            .rpc_endpoint
            .as_ref()
            .unwrap()
            .split(':')
            .last()
            .unwrap();

        let name = self.name();

        let _output = Command::new("docker")
            .args([
                "run",
                "-d",
                "--name",
                &name,
                "-p",
                &format!("{rpc_port}:26657"),
                "-p",
                "26656:26656",
                "-p",
                "1317:1317",
                "--mount",
                &format!("type=volume,source={}_data,target=/root", &name),
                "cosmwasm/wasmd:latest",
                "/opt/run_wasmd.sh",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        Ok(self)
    }

    pub fn wait_for_block(&self, ctx: AppContext) {
        ctx.rt.block_on(async {
            let query_client =
                QueryClient::new(self.chain_config.clone(), Some(ConnectionMode::Rpc))
                    .await
                    .unwrap();

            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                loop {
                    if query_client.block_height().await.unwrap_or_default() > 0 {
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            })
            .await
            .unwrap();
        });
    }

    fn clean(&self) {
        let name = self.name();
        let _ = Command::new("docker")
            .args(["kill", &name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();

        let _ = Command::new("docker")
            .args(["rm", &name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();

        let _ = Command::new("docker")
            .args(["volume", "rm", "-f", &format!("{}_data", name)])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();
    }
}

impl Drop for CosmosInstance {
    fn drop(&mut self) {
        self.clean();
    }
}
