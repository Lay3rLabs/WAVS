use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
};

use futures::{stream::FuturesUnordered, StreamExt};
use utils::{context::AppContext, filesystem::workspace_path, wkg::WkgClient};
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
        let wkg_client = WkgClient::new(configs.wavs.registry_domain.clone().unwrap()).unwrap();
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
                futures.push(get_digest(
                    http_client,
                    service_digest,
                    configs.registry,
                    &wkg_client,
                ));
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
    wkg_client: &WkgClient,
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
        let pkg_name = match name {
            DigestName::ChainTriggerLookup => "chain_trigger_lookup",
            DigestName::CosmosQuery => "cosmos_query",
            DigestName::EchoData => "echo_data",
            DigestName::Permissions => "permissions",
            DigestName::Square => "square",
        };
        let checksum_bytes = std::fs::read("../../checksums.txt").unwrap();
        let checksums_raw = std::str::from_utf8(&checksum_bytes).unwrap();
        let checksums: Vec<&str> = checksums_raw.split("\n").collect();
        let checksum = checksums
            .iter()
            .find(|check| {
                let path = check.split_ascii_whitespace().last().unwrap();
                let file_name = path.split("/").last().unwrap();
                let without_extension = file_name.split(".").next().unwrap();
                without_extension == pkg_name
            })
            .unwrap();
        let digest_string = checksum.split_ascii_whitespace().next().unwrap();
        let pkg_name = pkg_name.replace("_", "-");
        let bytes = wkg_client
            .fetch(&Registry {
                digest: Digest::from_str(digest_string).unwrap(),
                domain: None,
                version: None,
                package: PackageRef::try_from(format!("wavs-tests:{0}", pkg_name)).unwrap(),
            })
            .await
            .unwrap();
        (name, http_client.upload_component(bytes).await.unwrap())
    }
}
