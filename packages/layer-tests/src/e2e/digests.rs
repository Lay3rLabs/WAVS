use std::collections::{BTreeMap, HashSet};

use futures::{stream::FuturesUnordered, StreamExt};
use utils::{context::AppContext, filesystem::workspace_path};
use wasm_pkg_common::package::PackageRef;
use wavs_cli::clients::HttpClient;
use wavs_types::{Digest, Registry};

use super::config::Configs;

#[derive(Clone, Debug, Default)]
pub struct Digests {
    pub lookup: BTreeMap<DigestName, Digest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DigestName {
    ChainTriggerLookup,
    CosmosQuery,
    EchoData,
    Permissions,
    Square,
}

impl Digests {
    pub fn new(ctx: AppContext, configs: &Configs, http_client: &HttpClient) -> Self {
        ctx.rt.block_on(async {
            let digests: HashSet<DigestName> = configs
                .matrix
                .eth
                .iter()
                .map(|s| Vec::<DigestName>::from(*s))
                .chain(
                    configs
                        .matrix
                        .cosmos
                        .iter()
                        .map(|s| Vec::<DigestName>::from(*s)),
                )
                .chain(
                    configs
                        .matrix
                        .cross_chain
                        .iter()
                        .map(|s| Vec::<DigestName>::from(*s)),
                )
                .flatten()
                .collect();

            let mut futures = FuturesUnordered::new();

            for service_digest in digests {
                futures.push(get_digest(http_client, service_digest, configs.registry));
            }

            let mut lookup = BTreeMap::default();

            while let Some((name, digest)) = futures.next().await {
                lookup.insert(name, digest);
            }

            Self { lookup }
        })
    }
}

async fn get_digest(
    http_client: &HttpClient,
    name: DigestName,
    registry: bool,
) -> (DigestName, Digest) {
    if !registry {
        let wasm_filename = match name {
            DigestName::ChainTriggerLookup => "chain_trigger_lookup",
            DigestName::CosmosQuery => "cosmos_query",
            DigestName::EchoData => "echo_data",
            DigestName::Permissions => "permissions",
            DigestName::Square => "square",
        };

        let wasm_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join(format!("{}.wasm", wasm_filename));

        tracing::info!("Uploading wasm: {}", wasm_path.display());

        let wasm_bytes = tokio::fs::read(wasm_path).await.unwrap();

        (
            name,
            http_client
                .upload_component(wasm_bytes.to_vec())
                .await
                .unwrap(),
        )
    } else {
        let registry = match name {
            DigestName::ChainTriggerLookup => todo!(),
            DigestName::CosmosQuery => todo!(),
            DigestName::EchoData => todo!(),
            DigestName::Permissions => todo!(),
            DigestName::Square => Registry {
                domain: None,
                version: None,
                package: PackageRef::new(
                    "macovedj".to_string().try_into().unwrap(),
                    "square".to_string().try_into().unwrap(),
                ),
            },
        };
        (
            name,
            http_client
                .upload_component(serde_json::to_vec(&registry).unwrap())
                .await
                .unwrap(),
        )
    }
}
