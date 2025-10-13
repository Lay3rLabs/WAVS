use std::sync::Arc;

use anyhow::Result;
use utils::storage::db::RedbStorage;

use crate::config::Config;

pub struct UpgradeDb {
    pub upgraded: bool,
}

impl std::fmt::Display for UpgradeDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self.upgraded {
            true => "Upgraded database to the latest version",
            false => "Database is already up to date",
        };
        write!(f, "{}", message)
    }
}

pub struct UpgradeDbArgs {
    /// Path of the database file to upgrade relative to the data directory.
    pub db_path: String,
}

impl UpgradeDb {
    pub async fn run(config: &Config, args: UpgradeDbArgs) -> Result<Self> {
        let mut storage = RedbStorage::new(config.data.join(args.db_path))?;
        let upgraded = Arc::get_mut(&mut storage.inner).unwrap().upgrade()?;

        Ok(Self { upgraded })
    }
}
