use anyhow::Result;
use clap::Args;
use std::{net::SocketAddr, path::PathBuf};

use crate::operator::FileSystemOperator;

const DEFAULT_ADDR: std::net::SocketAddr = std::net::SocketAddr::new(
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
    8080,
);

/// Start up the Wasmatic server.
#[derive(Args)]
pub struct UpCommand {
    /// Socket address to bind the Operator API.
    #[clap(long = "bind", value_name = "BIND", default_value_t = DEFAULT_ADDR )]
    pub bind_addr: SocketAddr,

    /// The path to the parent storage directory to use.
    #[clap(long, value_name = "STORAGE_DIR", default_value = "data")]
    pub storage_dir: PathBuf,

    /// Global environment variables.
    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ' ')]
    pub envs: Vec<String>,
}

// TODO add path for config file option

impl UpCommand {
    /// Executes the command.
    pub async fn exec(self) -> Result<()> {
        let envs = self
            .envs
            .into_iter()
            .map(|s| {
                s.split_once('=')
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .ok_or(anyhow::anyhow!(
                        "invalid environment variable format: `{s}`"
                    ))
            })
            .collect::<Result<Vec<(String, String)>, _>>()?;
        let operator = FileSystemOperator::try_new(self.storage_dir, envs).await?;
        operator.serve(self.bind_addr).await?;
        Ok(())
    }
}
