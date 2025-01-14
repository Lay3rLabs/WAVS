use crate::{client::{try_get_cosmos_client, try_get_eigen_client}, config::Config};
use layer_climb::signing::SigningClient;
use utils::eigen_client::EigenClient;

use crate::{args::Command, deploy::Deployment};

pub struct ChainContext {
    // these are only used for on-chain commands
    pub eigen_client: Option<EigenClient>,
    pub deployment: Deployment,
    pub cosmos_client: Option<SigningClient>,
}

impl ChainContext {
    pub async fn try_new(command: &Command, config: &Config) -> Option<Self> {
        if matches!(command, Command::Exec { .. }) {
            return None;
        }

        let eigen_client = try_get_eigen_client(config).await;
        let cosmos_client = try_get_cosmos_client(config).await;

        let mut deployment = Deployment::load(config).unwrap();
        deployment
            .sanitize(command, config, eigen_client.as_ref(), cosmos_client.as_ref())
            .await
            .unwrap();

        Some(Self {
            eigen_client,
            cosmos_client,
            deployment,
        })
    }
}
