use anyhow::Result;
use dashmap::DashMap;
use futures::stream::{self};
use futures::StreamExt;
use reqwest::Client;
use std::collections::{BTreeSet, HashMap};
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::Mutex;
use wavs_types::aggregator::RegisterServiceRequest;

use utils::config::ChainConfigs;
use wavs_types::{ChainName, Service, Submit, Trigger, WorkflowID};

use super::clients::Clients;
use super::components::{ComponentName, ComponentSources};
use super::helpers;
use super::matrix::{CosmosService, CrossChainService, EvmService, TestMatrix};
use super::test_definition::{
    AggregatorDefinition, CosmosTriggerDefinition, EvmTriggerDefinition, OutputStructure,
    SubmitDefinition, TestBuilder, TestDefinition, WorkflowBuilder,
};
use crate::e2e::types::{CosmosQueryRequest, PermissionsRequest};

pub type CosmosCodeMap = Arc<DashMap<CosmosTriggerDefinition, Arc<Mutex<Option<u64>>>>>;

/// Registry for managing test definitions and their deployed services
#[derive(Default)]
pub struct TestRegistry {
    tests: HashMap<String, TestDefinition>,
}

/// Structure to hold the different chain names for test configuration
#[derive(Debug, Default, Clone)]
struct ChainNames {
    evm: Vec<ChainName>,
    evm_aggregator: Vec<(ChainName, String)>,
    cosmos: Vec<ChainName>,
}

impl ChainNames {
    /// Create a new ChainNames by categorizing chains from the config
    fn from_config(chain_configs: &ChainConfigs) -> Self {
        let mut chain_names = Self::default();

        // Categorize EVM chains
        for (chain_name, chain) in chain_configs.evm.iter() {
            if chain.aggregator_endpoint.is_some() {
                chain_names.evm_aggregator.push((
                    chain_name.clone(),
                    chain
                        .aggregator_endpoint
                        .clone()
                        .expect("Aggregator URL is expected"),
                ));
            } else {
                chain_names.evm.push(chain_name.clone());
            }
        }

        // Collect Cosmos chains
        chain_names.cosmos = chain_configs.cosmos.keys().cloned().collect::<Vec<_>>();

        chain_names
    }
}

impl TestRegistry {
    /// Create a new empty test registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a test definition
    pub fn register(&mut self, test: TestDefinition) -> &mut Self {
        // Store the test
        self.tests.insert(test.name.clone(), test);
        self
    }

    /// Get a test definition by name
    pub fn get(&self, name: &str) -> Option<&TestDefinition> {
        self.tests.get(name)
    }

    /// Get all test definitions
    pub fn list_all(&self) -> BTreeSet<&TestDefinition> {
        self.tests.values().collect()
    }

    /// Deploy services for all tests concurrently
    pub async fn deploy_services(
        &mut self,
        clients: &Clients,
        component_sources: &ComponentSources,
    ) -> Result<()> {
        let max_parallel = num_cpus::get();
        let cosmos_code_map = CosmosCodeMap::new(DashMap::new());

        let tests_iter = self.tests.iter_mut().map(|(_, test)| {
            let clients = clients.clone();
            let component_sources = component_sources.clone();
            let cosmos_triggers = cosmos_code_map.clone();
            async move {
                let service = helpers::deploy_service_for_test(
                    test,
                    &clients,
                    &component_sources,
                    cosmos_triggers,
                )
                .await?;

                for workflow in test.workflows.values() {
                    if let SubmitDefinition::Submit(Submit::Aggregator { url }) = &workflow.submit {
                        TestRegistry::register_to_aggregator(url, &service).await?;
                    }
                }

                test.service = Some(service);

                Ok::<(), anyhow::Error>(())
            }
        });

        let stream = stream::iter(tests_iter).buffer_unordered(max_parallel);

        tokio::pin!(stream);

        while let Some(result) = stream.next().await {
            if let Err(err) = result {
                tracing::error!("Test failed: {:?}", err);
            }
        }

        Ok(())
    }

    /// Registers a service on the aggregator
    pub async fn register_to_aggregator(url: &str, service: &Service) -> Result<()> {
        let http_client = Client::new();

        let endpoint = format!("{}/register-service", url);
        let payload = RegisterServiceRequest {
            service: service.clone(),
        };

        tracing::info!(
            "Registering service {} with aggregator at {}",
            service.id,
            endpoint
        );

        http_client
            .post(&endpoint)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    /// Create a registry based on the test mode
    pub fn from_test_mode(
        test_mode: crate::config::TestMode,
        chain_configs: &ChainConfigs,
    ) -> Result<Self> {
        // Convert TestMode to TestMatrix
        let matrix: TestMatrix = test_mode.clone().into();

        // Get chain names
        let chain_names = ChainNames::from_config(chain_configs);

        let mut registry = Self::new();

        // Register EVM tests
        if matrix.evm_regular_chain_enabled() && !chain_names.evm.is_empty() {
            let evm_chain = &chain_names.evm[0];

            // Basic EVM chain tests
            if matrix.evm.contains(&EvmService::EchoData) {
                registry.register_evm_echo_data_test(evm_chain)?;
            }

            if matrix.evm.contains(&EvmService::Square) {
                registry.register_evm_square_test(evm_chain)?;
            }

            if matrix.evm.contains(&EvmService::ChainTriggerLookup) {
                registry.register_evm_chain_trigger_lookup_test(evm_chain)?;
            }

            if matrix.evm.contains(&EvmService::Permissions) {
                registry.register_evm_permissions_test(evm_chain)?;
            }

            if matrix.evm.contains(&EvmService::MultiWorkflow) {
                registry.register_evm_multi_workflow_test(evm_chain)?;
            }

            if matrix.evm.contains(&EvmService::BlockInterval) {
                registry.register_evm_block_interval_test(evm_chain)?;
            }

            if matrix.evm.contains(&EvmService::CronInterval) {
                registry.register_evm_cron_interval_test(evm_chain)?;
            }
        }

        // Secondary chain tests
        if matrix.evm_secondary_chain_enabled() && chain_names.evm.len() > 1 {
            let trigger_chain = &chain_names.evm[0];
            let secondary_chain = &chain_names.evm[1];

            if matrix.evm.contains(&EvmService::EchoDataSecondaryChain) {
                registry
                    .register_evm_echo_data_secondary_chain_test(trigger_chain, secondary_chain)?;
            }
        }

        // Aggregator chain tests
        if matrix.evm_aggregator_chain_enabled() {
            let (aggregator_chain, url) = &chain_names.evm_aggregator[0];

            if matrix.evm.contains(&EvmService::EchoDataAggregator) {
                registry.register_evm_echo_data_aggregator_test(aggregator_chain, url)?;
            }
        }

        // Cosmos-related tests
        if matrix.cosmos_regular_chain_enabled()
            && !chain_names.cosmos.is_empty()
            && !chain_names.evm.is_empty()
        {
            let cosmos_chain = &chain_names.cosmos[0];
            let evm_chain = &chain_names.evm[0];

            // EVM tests that need Cosmos
            if matrix.evm.contains(&EvmService::CosmosQuery) {
                registry.register_evm_cosmos_query_test(evm_chain, cosmos_chain)?;
            }

            // Cosmos tests
            if matrix.cosmos.contains(&CosmosService::EchoData) {
                registry.register_cosmos_echo_data_test(cosmos_chain, evm_chain)?;
            }

            if matrix.cosmos.contains(&CosmosService::Square) {
                registry.register_cosmos_square_test(cosmos_chain, evm_chain)?;
            }

            if matrix.cosmos.contains(&CosmosService::ChainTriggerLookup) {
                registry.register_cosmos_chain_trigger_lookup_test(cosmos_chain, evm_chain)?;
            }

            if matrix.cosmos.contains(&CosmosService::CosmosQuery) {
                registry.register_cosmos_cosmos_query_test(cosmos_chain, evm_chain)?;
            }

            if matrix.cosmos.contains(&CosmosService::Permissions) {
                registry.register_cosmos_permissions_test(cosmos_chain, evm_chain)?;
            }

            if matrix.cosmos.contains(&CosmosService::BlockInterval) {
                registry.register_cosmos_block_interval_test(cosmos_chain, evm_chain)?;
            }

            if matrix.cosmos.contains(&CosmosService::CronInterval) {
                registry.register_cosmos_cron_interval_test(cosmos_chain, evm_chain)?;
            }

            // Cross-chain tests
            if matrix
                .cross_chain
                .contains(&CrossChainService::CosmosToEvmEchoData)
            {
                registry.register_cosmos_to_evm_echo_data_test(cosmos_chain, evm_chain)?;
            }
        }

        Ok(registry)
    }

    // Individual test registration methods (same as before)
    fn register_evm_echo_data_test(&mut self, chain: &ChainName) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_echo_data")
                .description("Tests the EchoData component on the primary EVM chain")
                .add_workflow(
                    WorkflowID::new("echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .evm_trigger(EvmTriggerDefinition::Simple {
                            chain_name: chain.clone(),
                        })
                        .evm_submit(chain)
                        .component(ComponentName::EchoData)
                        .input_text("The times")
                        .expect_same_output()
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_evm_echo_data_secondary_chain_test(
        &mut self,
        trigger_chain: &ChainName,
        chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_echo_data_secondary_chain")
                .description("Tests the EchoData component on the secondary EVM chain")
                .add_workflow(
                    WorkflowID::new("echo_data_secondary").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::EchoData)
                        .evm_trigger(EvmTriggerDefinition::Simple {
                            chain_name: trigger_chain.clone(),
                        })
                        .evm_submit(chain)
                        .input_text("collapse")
                        .expect_same_output()
                        .build()?,
                )?
                .service_manager_chain(chain)
                .build(),
        ))
    }

    fn register_evm_echo_data_aggregator_test(
        &mut self,
        aggregator_chain: &ChainName,
        url: &str,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_echo_data_aggregator")
                .description("Tests the EchoData component using an aggregator")
                .add_workflow(
                    WorkflowID::new("echo_data_aggregator").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::EchoData)
                        .evm_trigger(EvmTriggerDefinition::Simple {
                            chain_name: aggregator_chain.clone(),
                        })
                        .aggregator_submit(url)
                        .add_aggregator(AggregatorDefinition::NewEvmAggregatorSubmit {
                            chain_name: aggregator_chain.clone(),
                        })
                        .input_text("Chancellor")
                        .expect_same_output()
                        .build()?,
                )?
                .service_manager_chain(aggregator_chain)
                .build(),
        ))
    }

    fn register_evm_square_test(&mut self, chain: &ChainName) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_square")
                .description("Tests the Square component on EVM chain")
                .add_workflow(
                    WorkflowID::new("square").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::Square)
                        .evm_trigger(EvmTriggerDefinition::Simple {
                            chain_name: chain.clone(),
                        })
                        .evm_submit(chain)
                        .input_square(3)
                        .expect_square(9)
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_evm_chain_trigger_lookup_test(&mut self, chain: &ChainName) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_chain_trigger_lookup")
                .description("Tests the ChainTriggerLookup component on EVM chain")
                .add_workflow(
                    WorkflowID::new("chain_trigger_lookup").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::ChainTriggerLookup)
                        .evm_trigger(EvmTriggerDefinition::Simple {
                            chain_name: chain.clone(),
                        })
                        .evm_submit(chain)
                        .input_text("satoshi")
                        .expect_same_output()
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_evm_cosmos_query_test(
        &mut self,
        evm_chain: &ChainName,
        cosmos_chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_cosmos_query")
                .description("Tests the CosmosQuery component from EVM to Cosmos")
                .add_workflow(
                    WorkflowID::new("cosmos_query").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::CosmosQuery)
                        .evm_trigger(EvmTriggerDefinition::Simple {
                            chain_name: evm_chain.clone(),
                        })
                        .evm_submit(evm_chain)
                        .input_cosmos_query(CosmosQueryRequest::BlockHeight {
                            chain_name: cosmos_chain.clone(),
                        })
                        .expect_output_structure(OutputStructure::CosmosQueryResponse)
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_evm_permissions_test(&mut self, chain: &ChainName) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_permissions")
                .description("Tests permissions for HTTP and file system access on EVM chain")
                .add_workflow(
                    WorkflowID::new("permissions").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::Permissions)
                        .evm_trigger(EvmTriggerDefinition::Simple {
                            chain_name: chain.clone(),
                        })
                        .evm_submit(chain)
                        .input_permissions(create_permissions_request())
                        .expect_output_structure(OutputStructure::PermissionsResponse)
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_evm_multi_workflow_test(&mut self, chain: &ChainName) -> Result<&mut Self> {
        // This requires multiple workflows in a single test
        let test_builder = TestBuilder::new("evm_multi_workflow")
            .description("Tests multiple workflows in a single service on EVM chain");

        // Add first workflow (Square)
        let test_builder = test_builder.add_workflow(
            WorkflowID::new("square_workflow").unwrap(),
            WorkflowBuilder::new()
                .component(ComponentName::Square)
                .evm_trigger(EvmTriggerDefinition::Simple {
                    chain_name: chain.clone(),
                })
                .evm_submit(chain)
                .input_square(10)
                .expect_square(10)
                .build()?,
        )?;

        // Add second workflow (EchoData)
        let test_builder = test_builder.add_workflow(
            WorkflowID::new("echo_data_workflow").unwrap(),
            WorkflowBuilder::new()
                .component(ComponentName::EchoData)
                .evm_trigger(EvmTriggerDefinition::Simple {
                    chain_name: chain.clone(),
                })
                .evm_submit(chain)
                .input_square(10)
                .expect_same_output()
                .build()?,
        )?;

        // Complete the test definition
        Ok(self.register(test_builder.build()))
    }

    #[allow(dead_code)]
    fn register_evm_multi_trigger_test(
        &mut self,
        chain: &ChainName,
        trigger: Trigger,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_multi_trigger")
                .description("Tests multiple services triggered by the same event on EVM chain")
                .add_workflow(
                    WorkflowID::new("evm_multi_trigger").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::EchoData)
                        .trigger(trigger)
                        .evm_submit(chain)
                        .input_text("tttrrrrriiiigggeerrr")
                        .expect_same_output()
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_evm_block_interval_test(&mut self, chain: &ChainName) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_block_interval")
                .description("Tests the block interval trigger on EVM chain")
                .add_workflow(
                    WorkflowID::new("block_interval").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::EchoBlockInterval)
                        .block_interval_trigger(chain, NonZeroU32::new(1).unwrap(), None, None)
                        .evm_submit(chain)
                        .expect_text("block-interval data")
                        .build()?,
                )?
                .priority(0)
                .build(),
        ))
    }

    fn register_evm_cron_interval_test(&mut self, chain: &ChainName) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("evm_cron_interval")
                .description("Tests the cron interval trigger")
                .add_workflow(
                    WorkflowID::new("cron_interval").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::EchoCronInterval)
                        .cron_trigger("* * * * * *", None, None)
                        .evm_submit(chain)
                        .expect_text("cron-interval data")
                        .build()?,
                )?
                .priority(1)
                .build(),
        ))
    }

    // Cosmos test registrations

    fn register_cosmos_echo_data_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("cosmos_echo_data")
                .description("Tests the EchoData component on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::EchoData)
                        .cosmos_trigger(CosmosTriggerDefinition::Simple {
                            chain_name: cosmos_chain.clone(),
                        })
                        .evm_submit(evm_chain)
                        .input_text("on brink")
                        .expect_same_output()
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_cosmos_square_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("cosmos_square")
                .description("Tests the Square component on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_square").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::Square)
                        .cosmos_trigger(CosmosTriggerDefinition::Simple {
                            chain_name: cosmos_chain.clone(),
                        })
                        .evm_submit(evm_chain)
                        .input_square(3)
                        .expect_square(9)
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_cosmos_chain_trigger_lookup_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("cosmos_chain_trigger_lookup")
                .description("Tests the ChainTriggerLookup component on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_chain_trigger_lookup").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::ChainTriggerLookup)
                        .cosmos_trigger(CosmosTriggerDefinition::Simple {
                            chain_name: cosmos_chain.clone(),
                        })
                        .evm_submit(evm_chain)
                        .input_text("nakamoto")
                        .expect_same_output()
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_cosmos_cosmos_query_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("cosmos_cosmos_query")
                .description("Tests the CosmosQuery component on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_cosmos_query").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::CosmosQuery)
                        .cosmos_trigger(CosmosTriggerDefinition::Simple {
                            chain_name: cosmos_chain.clone(),
                        })
                        .evm_submit(evm_chain)
                        .input_cosmos_query(CosmosQueryRequest::BlockHeight {
                            chain_name: cosmos_chain.clone(),
                        })
                        .expect_output_structure(OutputStructure::CosmosQueryResponse)
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_cosmos_permissions_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("cosmos_permissions")
                .description("Tests permissions for HTTP and file system access on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_permissions").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::Permissions)
                        .cosmos_trigger(CosmosTriggerDefinition::Simple {
                            chain_name: cosmos_chain.clone(),
                        })
                        .evm_submit(evm_chain)
                        .input_permissions(create_permissions_request())
                        .expect_output_structure(OutputStructure::PermissionsResponse)
                        .build()?,
                )?
                .build(),
        ))
    }

    fn register_cosmos_block_interval_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("cosmos_block_interval")
                .description("Tests the block interval trigger on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_block_interval").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::EchoBlockInterval)
                        .block_interval_trigger(
                            cosmos_chain,
                            NonZeroU32::new(1).unwrap(),
                            None,
                            None,
                        )
                        .evm_submit(evm_chain)
                        .expect_text("block-interval data")
                        .build()?,
                )?
                .priority(0)
                .build(),
        ))
    }

    fn register_cosmos_cron_interval_test(
        &mut self,
        _cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("cosmos_cron_interval")
                .description("Tests the cron interval trigger on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_cron_interval").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::EchoCronInterval)
                        .cron_trigger("* * * * * *", None, None)
                        .evm_submit(evm_chain)
                        .expect_text("cron-interval data")
                        .build()?,
                )?
                .priority(1)
                .build(),
        ))
    }

    // Cross-chain test registrations

    fn register_cosmos_to_evm_echo_data_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> Result<&mut Self> {
        Ok(self.register(
            TestBuilder::new("cross_chain_cosmos_to_evm_echo_data")
                .description("Tests the EchoData component from Cosmos to EVM")
                .add_workflow(
                    WorkflowID::new("cross_chain_echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .component(ComponentName::EchoData)
                        .cosmos_trigger(CosmosTriggerDefinition::Simple {
                            chain_name: cosmos_chain.clone(),
                        })
                        .evm_submit(evm_chain)
                        .input_text("hello EVM world from cosmos")
                        .expect_same_output()
                        .build()?,
                )?
                .build(),
        ))
    }
}

// Helper function to create a standard permissions request for tests
fn create_permissions_request() -> PermissionsRequest {
    PermissionsRequest {
        get_url: "https://postman-echo.com/get".to_string(),
        post_url: "https://postman-echo.com/post".to_string(),
        post_data: ("hello".to_string(), "world".to_string()),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}
