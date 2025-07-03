use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
};

use futures::{stream::FuturesUnordered, StreamExt};
use utils::filesystem::workspace_path;
use wasm_pkg_common::package::PackageRef;
use wavs_cli::clients::HttpClient;
use wavs_types::{ComponentSource, Digest, Registry};

use super::config::Configs;

#[derive(Clone, Debug, Default)]
pub struct ComponentSources {
    pub lookup: BTreeMap<ComponentName, ComponentSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ComponentName {
    ChainTriggerLookup,
    CosmosQuery,
    EchoData,
    KvStore,
    KvWriter,
    KvReader,
    Permissions,
    Square,
    EchoBlockInterval,
    EchoCronInterval,
}

impl ComponentName {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComponentName::ChainTriggerLookup => "chain_trigger_lookup",
            ComponentName::CosmosQuery => "cosmos_query",
            ComponentName::EchoData => "echo_data",
            ComponentName::KvStore => "kv_store",
            ComponentName::KvWriter => "kv_writer",
            ComponentName::KvReader => "kv_reader",
            ComponentName::Permissions => "permissions",
            ComponentName::Square => "square",
            ComponentName::EchoBlockInterval => "echo_block_interval",
            ComponentName::EchoCronInterval => "echo_cron_interval",
        }
    }
}

impl ComponentSources {
    pub async fn new(configs: &Configs, http_client: &HttpClient) -> Self {
        let component_names: HashSet<ComponentName> = configs
            .matrix
            .evm
            .iter()
            .map(|s| Vec::<ComponentName>::from(*s))
            .chain(
                configs
                    .matrix
                    .cosmos
                    .iter()
                    .map(|s| Vec::<ComponentName>::from(*s)),
            )
            .chain(
                configs
                    .matrix
                    .cross_chain
                    .iter()
                    .map(|s| Vec::<ComponentName>::from(*s)),
            )
            .flatten()
            .collect();

        let mut futures = FuturesUnordered::new();

        for component_name in component_names {
            futures.push(get_component_source(
                http_client,
                component_name,
                configs.registry,
            ));
        }

        let mut lookup = BTreeMap::default();

        while let Some((name, digest)) = futures.next().await {
            lookup.insert(name, digest);
        }

        Self { lookup }
    }
}

async fn get_component_source(
    http_client: &HttpClient,
    name: ComponentName,
    registry: bool,
) -> (ComponentName, ComponentSource) {
    if !registry {
        let wasm_filename = name.as_str();

        let wasm_path = workspace_path()
            .join("examples")
            .join("build")
            .join("components")
            .join(format!("{}.wasm", wasm_filename));

        tracing::info!("Uploading wasm: {}", wasm_path.display());

        let wasm_bytes = tokio::fs::read(wasm_path).await.unwrap();

        let digest = http_client
            .upload_component(wasm_bytes.to_vec())
            .await
            .unwrap();
        (name, ComponentSource::Digest(digest))
    } else {
        // Adding a component from the registry requires calculating the digest ahead-of-time.
        // While we could do that by either loading the files from disk or downloading from the registry
        // and calculating the hash from that - using the checksums is faster and gives us an extra
        // sanity check that we've deployed the latest builds to the test registry
        let pkg_name = name.as_str();
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

        let source = ComponentSource::Registry {
            registry: Registry {
                digest: Digest::from_str(digest_string).unwrap(),
                domain: None,
                version: None,
                package: PackageRef::try_from(format!("wavs-tests:{0}", pkg_name)).unwrap(),
            },
        };

        (name, source)
    }
}
