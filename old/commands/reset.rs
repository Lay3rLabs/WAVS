use anyhow::Result;
use clap::Args;
use std::{fs, path::PathBuf};

use crate::config::WasmaticConfig;
use crate::storage::{FileSystemStorage, Storage};

/// Reset Wasmatic server data.
#[derive(Args)]
pub struct ResetCommand {
    /// The path to the config file.
    #[clap(long, value_name = "CONFIG", default_value = "wasmatic.toml")]
    pub config: PathBuf,

    /// The path to the parent storage directory to use.
    #[clap(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
}

// TODO add dialogue that asks the user to confirm?

impl ResetCommand {
    /// Executes the command.
    pub async fn exec(self) -> Result<()> {
        let config: WasmaticConfig = toml::from_str(&fs::read_to_string(&self.config).or_else(
            |_| -> Result<String> {
                fs::write(&self.config, "").unwrap();
                Ok("".to_string())
            },
        )?)?;

        // use CLI (if provided) or provided in the `wasmatic.toml` or the `./data` dir
        let dir = self.dir.or(config.dir).unwrap_or(PathBuf::from("data"));

        let storage = match FileSystemStorage::try_lock(&dir).await? {
            Some(storage) => storage,
            None => {
                return Err(anyhow::anyhow!(
                    "unable to acquire file system lock for path: {base_dir}",
                    base_dir = dir.display()
                ))
            }
        };
        storage.reset().await?;

        Ok(())
    }
}
