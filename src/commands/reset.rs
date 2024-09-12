use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::storage::{FileSystemStorage, Storage};

/// Reset Wasmatic server data.
#[derive(Args)]
pub struct ResetCommand {
    /// The path to the parent storage directory to use.
    #[clap(long, value_name = "STORAGE_DIR", default_value = "data")]
    pub storage_dir: PathBuf,
}

// TODO add path for config file option

impl ResetCommand {
    /// Executes the command.
    pub async fn exec(self) -> Result<()> {
        let storage = match FileSystemStorage::try_lock(&self.storage_dir).await? {
            Some(storage) => storage,
            None => {
                return Err(anyhow::anyhow!(
                    "unable to acquire file system lock for path: {base_dir}",
                    base_dir = self.storage_dir.display()
                ))
            }
        };
        storage.reset().await?;

        Ok(())
    }
}
