use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;

use utils::config::ChainConfigs;
use wavs_types::ChainName;

use super::clients::Clients;
use super::components::{ComponentName, ComponentSources};
use super::helpers;
use super::matrix::{CosmosService, CrossChainService, EvmService, TestMatrix};
use super::test_definition::{ExpectedOutput, TestBuilder, TestDefinition};
use crate::e2e::types::{CosmosQueryRequest, PermissionsRequest};

/// Registry for managing test definitions and their deployed services
pub struct TestRegistry {
    tests: HashMap<String, TestDefinition>,
}

/// Structure to hold the different chain names for test configuration
#[derive(Debug, Default, Clone)]
struct ChainNames {
    evm: Vec<ChainName>,
    evm_aggregator: Vec<ChainName>,
    cosmos: Vec<ChainName>,
}

impl ChainNames {
    /// Create a new ChainNames by categorizing chains from the config
    fn from_config(chain_configs: &ChainConfigs) -> Self {
        let mut chain_names = Self::default();

        // Categorize EVM chains
        for (chain_name, chain) in chain_configs.evm.iter() {
            if chain.aggregator_endpoint.is_some() {
                chain_names.evm_aggregator.push(chain_name.clone());
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
        Self {
            tests: HashMap::new(),
        }
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
    pub fn list_all(&self) -> Vec<&TestDefinition> {
        self.tests.values().collect()
    }

    /// Deploy services for all tests
    pub async fn deploy_services(
        &mut self,
        clients: &Clients,
        component_sources: &ComponentSources,
    ) -> Result<()> {
        // Process each test one at a time
        for test in self.tests.values_mut() {
            // Deploy service for this test
            let (service, multi_trigger_service) =
                helpers::deploy_service_for_test(test, clients, component_sources).await?;

            // If deployment was successful, attach services to the test
            test.service = Some(service);
            test.multi_trigger_service = multi_trigger_service;
        }

        Ok(())
    }

    /// Create a registry based on the test mode
    pub fn from_test_mode(
        test_mode: &crate::config::TestMode,
        chain_configs: &ChainConfigs,
    ) -> Self {
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
                registry.register_evm_echo_data_test(evm_chain);
            }

            if matrix.evm.contains(&EvmService::Square) {
                registry.register_evm_square_test(evm_chain);
            }

            if matrix.evm.contains(&EvmService::ChainTriggerLookup) {
                registry.register_evm_chain_trigger_lookup_test(evm_chain);
            }

            if matrix.evm.contains(&EvmService::Permissions) {
                registry.register_evm_permissions_test(evm_chain);
            }

            if matrix.evm.contains(&EvmService::MultiWorkflow) {
                registry.register_evm_multi_workflow_test(evm_chain);
            }

            if matrix.evm.contains(&EvmService::MultiTrigger) {
                registry.register_evm_multi_trigger_test(evm_chain);
            }

            if matrix.evm.contains(&EvmService::BlockInterval) {
                registry.register_evm_block_interval_test(evm_chain);
            }

            if matrix.evm.contains(&EvmService::CronInterval) {
                registry.register_evm_cron_interval_test(evm_chain);
            }
        }

        // Secondary chain tests
        if matrix.evm_secondary_chain_enabled() && chain_names.evm.len() > 1 {
            let secondary_chain = &chain_names.evm[1];

            if matrix.evm.contains(&EvmService::EchoDataSecondaryChain) {
                registry.register_evm_echo_data_secondary_chain_test(secondary_chain);
            }
        }

        // Aggregator chain tests
        if matrix.evm_aggregator_chain_enabled() && !chain_names.evm_aggregator.is_empty() {
            let aggregator_chain = &chain_names.evm_aggregator[0];

            if matrix.evm.contains(&EvmService::EchoDataAggregator) {
                registry.register_evm_echo_data_aggregator_test(aggregator_chain);
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
                registry.register_evm_cosmos_query_test(evm_chain, cosmos_chain);
            }

            // Cosmos tests
            if matrix.cosmos.contains(&CosmosService::EchoData) {
                registry.register_cosmos_echo_data_test(cosmos_chain, evm_chain);
            }

            if matrix.cosmos.contains(&CosmosService::Square) {
                registry.register_cosmos_square_test(cosmos_chain, evm_chain);
            }

            if matrix.cosmos.contains(&CosmosService::ChainTriggerLookup) {
                registry.register_cosmos_chain_trigger_lookup_test(cosmos_chain, evm_chain);
            }

            if matrix.cosmos.contains(&CosmosService::CosmosQuery) {
                registry.register_cosmos_cosmos_query_test(cosmos_chain, evm_chain);
            }

            if matrix.cosmos.contains(&CosmosService::Permissions) {
                registry.register_cosmos_permissions_test(cosmos_chain, evm_chain);
            }

            if matrix.cosmos.contains(&CosmosService::BlockInterval) {
                registry.register_cosmos_block_interval_test(cosmos_chain, evm_chain);
            }

            if matrix.cosmos.contains(&CosmosService::CronInterval) {
                registry.register_cosmos_cron_interval_test(cosmos_chain, evm_chain);
            }

            // Cross-chain tests
            if matrix
                .cross_chain
                .contains(&CrossChainService::CosmosToEvmEchoData)
            {
                registry.register_cosmos_to_evm_echo_data_test(cosmos_chain, evm_chain);
            }
        }

        registry
    }

    // Individual test registration methods (same as before)
    fn register_evm_echo_data_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_echo_data")
                .description("Tests the EchoData component on the primary EVM chain")
                .component(ComponentName::EchoData)
                .evm_trigger(chain.as_ref())
                .evm_submit(chain.as_ref())
                .input_text("The times")
                .expect_same_output()
                .build(),
        )
    }

    fn register_evm_echo_data_secondary_chain_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_echo_data_secondary_chain")
                .description("Tests the EchoData component on the secondary EVM chain")
                .component(ComponentName::EchoData)
                .evm_trigger(chain.as_ref())
                .evm_submit(chain.as_ref())
                .input_text("collapse")
                .expect_same_output()
                .build(),
        )
    }

    fn register_evm_echo_data_aggregator_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_echo_data_aggregator")
                .description("Tests the EchoData component using an aggregator")
                .component(ComponentName::EchoData)
                .evm_trigger(chain.as_ref())
                .aggregator_submit(chain.as_ref())
                .input_text("Chancellor")
                .expect_same_output()
                .num_tasks(3)
                .build(),
        )
    }

    fn register_evm_square_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_square")
                .description("Tests the Square component on EVM chain")
                .component(ComponentName::Square)
                .evm_trigger(chain.as_ref())
                .evm_submit(chain.as_ref())
                .input_square(3)
                .expect_square(9)
                .build(),
        )
    }

    fn register_evm_chain_trigger_lookup_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_chain_trigger_lookup")
                .description("Tests the ChainTriggerLookup component on EVM chain")
                .component(ComponentName::ChainTriggerLookup)
                .evm_trigger(chain.as_ref())
                .evm_submit(chain.as_ref())
                .input_text("satoshi")
                .expect_same_output()
                .build(),
        )
    }

    fn register_evm_cosmos_query_test(
        &mut self,
        evm_chain: &ChainName,
        cosmos_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("evm_cosmos_query")
                .description("Tests the CosmosQuery component from EVM to Cosmos")
                .component(ComponentName::CosmosQuery)
                .evm_trigger(evm_chain.as_ref())
                .evm_submit(evm_chain.as_ref())
                .input_cosmos_query(CosmosQueryRequest::BlockHeight {
                    chain_name: cosmos_chain.clone(),
                })
                .expect_output_structure(ExpectedOutput::StructureOnly(
                    super::test_definition::OutputStructure::CosmosQueryResponse,
                ))
                .build(),
        )
    }

    fn register_evm_permissions_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_permissions")
                .description("Tests permissions for HTTP and file system access on EVM chain")
                .component(ComponentName::Permissions)
                .evm_trigger(chain.as_ref())
                .evm_submit(chain.as_ref())
                .input_permissions(create_permissions_request())
                .expect_output_structure(ExpectedOutput::StructureOnly(
                    super::test_definition::OutputStructure::PermissionsResponse,
                ))
                .build(),
        )
    }

    fn register_evm_multi_workflow_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_multi_workflow")
                .description("Tests multiple workflows in a single service on EVM chain")
                .components(vec![ComponentName::Square, ComponentName::EchoData])
                .evm_trigger(chain.as_ref())
                .evm_submit(chain.as_ref())
                .build(),
        )
    }

    fn register_evm_multi_trigger_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_multi_trigger")
                .description("Tests multiple services triggered by the same event on EVM chain")
                .component(ComponentName::EchoData)
                .evm_trigger(chain.as_ref())
                .evm_submit(chain.as_ref())
                .input_text("tttrrrrriiiigggeerrr")
                .expect_same_output()
                .with_multi_trigger()
                .build(),
        )
    }

    fn register_evm_block_interval_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_block_interval")
                .description("Tests the block interval trigger on EVM chain")
                .component(ComponentName::EchoBlockInterval)
                .block_interval_trigger(chain.as_ref(), 1)
                .evm_submit(chain.as_ref())
                .timeout(Duration::from_secs(15))
                .build(),
        )
    }

    fn register_evm_cron_interval_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_cron_interval")
                .description("Tests the cron interval trigger")
                .component(ComponentName::EchoCronInterval)
                .cron_trigger("* * * * * *")
                .evm_submit(chain.as_ref())
                .timeout(Duration::from_secs(15))
                .build(),
        )
    }

    // Cosmos test registrations

    fn register_cosmos_echo_data_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_echo_data")
                .description("Tests the EchoData component on Cosmos chain")
                .component(ComponentName::EchoData)
                .cosmos_trigger(cosmos_chain.as_ref())
                .evm_submit(evm_chain.as_ref())
                .input_text("on brink")
                .expect_same_output()
                .build(),
        )
    }

    fn register_cosmos_square_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_square")
                .description("Tests the Square component on Cosmos chain")
                .component(ComponentName::Square)
                .cosmos_trigger(cosmos_chain.as_ref())
                .evm_submit(evm_chain.as_ref())
                .input_square(3)
                .expect_square(9)
                .build(),
        )
    }

    fn register_cosmos_chain_trigger_lookup_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_chain_trigger_lookup")
                .description("Tests the ChainTriggerLookup component on Cosmos chain")
                .component(ComponentName::ChainTriggerLookup)
                .cosmos_trigger(cosmos_chain.as_ref())
                .evm_submit(evm_chain.as_ref())
                .input_text("nakamoto")
                .expect_same_output()
                .build(),
        )
    }

    fn register_cosmos_cosmos_query_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_cosmos_query")
                .description("Tests the CosmosQuery component on Cosmos chain")
                .component(ComponentName::CosmosQuery)
                .cosmos_trigger(cosmos_chain.as_ref())
                .evm_submit(evm_chain.as_ref())
                .input_cosmos_query(CosmosQueryRequest::BlockHeight {
                    chain_name: cosmos_chain.clone(),
                })
                .expect_output_structure(ExpectedOutput::StructureOnly(
                    super::test_definition::OutputStructure::CosmosQueryResponse,
                ))
                .build(),
        )
    }

    fn register_cosmos_permissions_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_permissions")
                .description("Tests permissions for HTTP and file system access on Cosmos chain")
                .component(ComponentName::Permissions)
                .cosmos_trigger(cosmos_chain.as_ref())
                .evm_submit(evm_chain.as_ref())
                .input_permissions(create_permissions_request())
                .expect_output_structure(ExpectedOutput::StructureOnly(
                    super::test_definition::OutputStructure::PermissionsResponse,
                ))
                .build(),
        )
    }

    fn register_cosmos_block_interval_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_block_interval")
                .description("Tests the block interval trigger on Cosmos chain")
                .component(ComponentName::EchoBlockInterval)
                .block_interval_trigger(cosmos_chain.as_ref(), 1)
                .evm_submit(evm_chain.as_ref())
                .timeout(Duration::from_secs(15))
                .build(),
        )
    }

    fn register_cosmos_cron_interval_test(
        &mut self,
        _cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_cron_interval")
                .description("Tests the cron interval trigger on Cosmos chain")
                .component(ComponentName::EchoCronInterval)
                .cron_trigger("* * * * * *")
                .evm_submit(evm_chain.as_ref())
                .timeout(Duration::from_secs(15))
                .build(),
        )
    }

    // Cross-chain test registrations

    fn register_cosmos_to_evm_echo_data_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cross_chain_cosmos_to_evm_echo_data")
                .description("Tests the EchoData component from Cosmos to EVM")
                .component(ComponentName::EchoData)
                .cosmos_trigger(cosmos_chain.as_ref())
                .evm_submit(evm_chain.as_ref())
                .input_text("hello EVM world from cosmos")
                .expect_same_output()
                .build(),
        )
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

// Add extension trait for TestBuilder to support output structure expectation
pub trait TestBuilderExt {
    fn expect_output_structure(self, structure: ExpectedOutput) -> Self;
}

impl TestBuilderExt for TestBuilder {
    fn expect_output_structure(mut self, structure: ExpectedOutput) -> Self {
        match structure {
            ExpectedOutput::StructureOnly(_) => {
                // We can assign directly since we're checking it's the right variant
                self.definition.expected_output = structure;
                self
            }
            _ => panic!("expect_output_structure can only be used with StructureOnly variant"),
        }
    }
}
