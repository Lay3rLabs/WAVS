use anyhow::Result;
use clap::Args;
use std::{fs, net::SocketAddr, path::PathBuf};

use crate::config::WasmaticConfig;
use crate::operator::FileSystemOperator;

const DEFAULT_ADDR: std::net::SocketAddr = std::net::SocketAddr::new(
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
    8081,
);

/// Start up the Wasmatic server.
#[derive(Args)]
pub struct UpCommand {
    /// The path to the config file.
    #[clap(long, value_name = "CONFIG", default_value = "wasmatic.toml")]
    pub config: PathBuf,

    /// Socket address to bind the Operator API.
    #[clap(long = "bind", value_name = "BIND")]
    pub bind: Option<SocketAddr>,

    /// The path to the parent storage directory to use.
    #[clap(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,

    /// Global environment variables.
    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ' ')]
    pub envs: Vec<String>,
}

impl UpCommand {
    /// Executes the command.
    pub async fn exec(self) -> Result<()> {
        let config: WasmaticConfig = toml::from_str(&fs::read_to_string(&self.config).or_else(
            |_| -> Result<String> {
                fs::write(&self.config, "").unwrap();
                Ok("".to_string())
            },
        )?)?;

        // use CLI (if provided) or provided in the `wasmatic.toml` or the `DEFAULT_ADDR`
        let bind = self.bind.or(config.bind).unwrap_or(DEFAULT_ADDR);

        // use CLI (if provided) or provided in the `wasmatic.toml` or the `./data` dir
        let dir = self
            .dir
            .or(config.dir.clone())
            .unwrap_or(PathBuf::from("data"));

        // join CLI env vars with those provided in the `wasmatic.toml`
        let envs = config
            .envs
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|[k, v]| Ok((k, v)))
            .chain(self.envs.into_iter().map(|s| {
                s.split_once('=')
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .ok_or(anyhow::anyhow!(
                        "invalid environment variable format: `{s}`"
                    ))
            }))
            .collect::<Result<Vec<(String, String)>, _>>()?;

        let operator = FileSystemOperator::try_new(dir, envs, config).await?;
        operator.serve(bind).await?;
        Ok(())
    }
}
