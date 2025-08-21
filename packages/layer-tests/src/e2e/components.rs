use std::{
    collections::{BTreeMap, HashMap, HashSet},
    str::FromStr,
};

use super::{config::Configs, test_definition::SubmitDefinition, test_registry::TestRegistry};
use futures::{stream::FuturesUnordered, StreamExt};
use utils::filesystem::workspace_path;
use wasm_pkg_common::package::PackageRef;
use wavs_aggregator::config::Config as AggregatorConfig;
use wavs_cli::clients::HttpClient;
use wavs_types::{ComponentDigest, ComponentSource, Registry};

#[derive(Clone, Debug, Default)]
pub struct ComponentSources {
    pub lookup: BTreeMap<ComponentName, ComponentSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ComponentType {
    Operator,
    Aggregator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ComponentName {
    // Operator components
    ChainTriggerLookup,
    CosmosQuery,
    KvStore,
    EchoData,
    Permissions,
    Square,
    EchoBlockInterval,
    EchoCronInterval,
    // Aggregator components
    SimpleAggregator,
    TimerAggregator,
}

impl ComponentName {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComponentName::ChainTriggerLookup => "chain_trigger_lookup",
            ComponentName::CosmosQuery => "cosmos_query",
            ComponentName::KvStore => "kv_store",
            ComponentName::EchoData => "echo_data",
            ComponentName::Permissions => "permissions",
            ComponentName::Square => "square",
            ComponentName::EchoBlockInterval => "echo_block_interval",
            ComponentName::EchoCronInterval => "echo_cron_interval",
            ComponentName::SimpleAggregator => "simple_aggregator",
            ComponentName::TimerAggregator => "timer_aggregator",
        }
    }

    pub fn component_type(&self) -> ComponentType {
        match self {
            ComponentName::SimpleAggregator | ComponentName::TimerAggregator => {
                ComponentType::Aggregator
            }
            _ => ComponentType::Operator,
        }
    }

    pub fn is_aggregator(&self) -> bool {
        self.component_type() == ComponentType::Aggregator
    }

    pub fn is_operator(&self) -> bool {
        self.component_type() == ComponentType::Operator
    }
}

impl ComponentSources {
    pub async fn new(
        configs: &Configs,
        registry: &TestRegistry,
        http_client: &HttpClient,
        aggregator_clients: &[HttpClient],
        aggregator_configs: &[AggregatorConfig],
    ) -> Self {
        let mut component_names: HashSet<ComponentName> = configs
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

        // Collect which aggregator components need to go to which aggregator endpoint
        let mut aggregator_components_by_endpoint: HashMap<String, HashSet<ComponentName>> =
            HashMap::new();
        for test in registry.list_all() {
            for workflow in test.workflows.values() {
                let SubmitDefinition::Aggregator { url, .. } = &workflow.submit;
                for aggregator in &workflow.aggregators {
                    aggregator_components_by_endpoint
                        .entry(url.clone())
                        .or_default()
                        .insert(*aggregator);
                    component_names.insert(*aggregator);
                }
            }
        }

        let mut futures = FuturesUnordered::new();

        for component_name in component_names {
            futures.push(get_component_source(
                http_client,
                aggregator_clients,
                aggregator_configs,
                component_name,
                configs.registry,
                &aggregator_components_by_endpoint,
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
    aggregator_clients: &[HttpClient],
    aggregator_configs: &[AggregatorConfig],
    name: ComponentName,
    registry: bool,
    aggregator_components_by_endpoint: &HashMap<String, HashSet<ComponentName>>,
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

        let digest = if name.is_aggregator() {
            let mut digest = None;

            // Upload to each aggregator that has this component specified
            for (client, config) in aggregator_clients.iter().zip(aggregator_configs.iter()) {
                let endpoint_url = format!("http://{}:{}", config.host, config.port);

                if let Some(components) = aggregator_components_by_endpoint.get(&endpoint_url) {
                    if components.contains(&name) {
                        tracing::info!("Uploading {} to {}", name.as_str(), endpoint_url);
                        let uploaded_digest =
                            client.upload_component(wasm_bytes.to_vec()).await.unwrap();

                        if let Some(existing_digest) = &digest {
                            assert_eq!(existing_digest, &uploaded_digest,
                                "Different aggregators returned different digests for the same component");
                        } else {
                            digest = Some(uploaded_digest);
                        }
                    }
                }
            }
            digest.expect("No aggregator clients available")
        } else {
            // Operator components go to WAVS server
            http_client
                .upload_component(wasm_bytes.to_vec())
                .await
                .unwrap()
        };
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
                digest: ComponentDigest::from_str(digest_string).unwrap(),
                domain: None,
                version: None,
                package: PackageRef::try_from(format!("wavs-tests:{0}", pkg_name)).unwrap(),
            },
        };

        (name, source)
    }
}
