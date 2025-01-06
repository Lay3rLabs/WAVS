use crate::{client::get_eigen_client, config::Config};
use utils::eigen_client::EigenClient;

use crate::{args::Command, deploy::Deployment};

pub struct ChainContext {
    // these are only used for on-chain commands
    pub eigen_client: EigenClient,
    pub deployment: Deployment,
}

impl ChainContext {
    pub async fn try_new(command: &Command, config: &Config) -> Option<Self> {
        if matches!(command, Command::Exec { .. }) {
            return None;
        }

        let eigen_client = get_eigen_client(config).await;
        let mut deployment = Deployment::load(config).unwrap();
        deployment
            .sanitize(command, config, &eigen_client)
            .await
            .unwrap();

        Some(Self {
            eigen_client,
            deployment,
        })
    }
}
