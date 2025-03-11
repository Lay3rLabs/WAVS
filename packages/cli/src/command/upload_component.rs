use anyhow::Result;
use wavs_types::Digest;

use crate::{clients::HttpClient, config::Config, util::read_component};

pub struct UploadComponent {
    pub digest: Digest,
}

impl std::fmt::Display for UploadComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Digest: \n{}", self.digest)
    }
}

pub struct UploadComponentArgs {
    pub component_path: String,
}

impl UploadComponent {
    pub async fn run(
        config: &Config,
        UploadComponentArgs { component_path }: UploadComponentArgs,
    ) -> Result<Self> {
        let wasm_bytes = read_component(&component_path)?;
        let http_client = HttpClient::new(config.wavs_endpoint.clone());
        let digest = http_client.upload_component(wasm_bytes).await?;

        Ok(Self { digest })
    }
}
