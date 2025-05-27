use anyhow::Result;
use dashmap::DashMap;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use regex::Regex;
use reqwest::Client;
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::Mutex;
use wavs_types::aggregator::RegisterServiceRequest;

use utils::config::ChainConfigs;
use wavs_types::{ChainName, Service, Submit, Trigger, WorkflowID};

use super::chain_names::ChainNames;
use super::clients::Clients;
use super::components::{ComponentName, ComponentSources};
use super::config::{BLOCK_INTERVAL_DATA_PREFIX, CRON_INTERVAL_DATA};
use super::helpers;
use super::matrix::{CosmosService, CrossChainService, EvmService, TestMatrix};
use super::test_definition::{
    AggregatorDefinition, CosmosTriggerDefinition, EvmTriggerDefinition, ExpectedOutput, InputData,
    OutputStructure, SubmitDefinition, TestBuilder, TestDefinition, TriggerDefinition,
    WorkflowBuilder,
};
use crate::e2e::types::{CosmosQueryRequest, PermissionsRequest};

/// This map is used to ensure cosmos contracts only have their wasm uploaded once
/// Key -> Cosmos Trigger Definition, Value -> Maybe Code Id
pub type CosmosTriggerCodeMap = Arc<DashMap<CosmosTriggerDefinition, Arc<Mutex<Option<u64>>>>>;

// Eventually we will have multiple aggregators to test against, but for now we use a single local aggregator
const AGGREGATOR_ENDPOINT: &str = "http://127.0.0.1:8001";

/// Registry for managing test definitions and their deployed services
#[derive(Default)]
pub struct TestRegistry {
    tests: Vec<TestDefinition>,
}

impl TestRegistry {
    /// Create a new empty test registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a test definition
    pub fn register(&mut self, test: TestDefinition) -> &mut Self {
        // Store the test
        self.tests.push(test);
        self
    }

    /// Group all test definitions by group (ascending priority)
    pub fn list_all_grouped(&self) -> BTreeMap<u64, Vec<&TestDefinition>> {
        let mut map: BTreeMap<u64, Vec<&TestDefinition>> = BTreeMap::new();
        for test in &self.tests {
            map.entry(test.group).or_default().push(test);
        }
        map
    }

    /// Deploy services for all tests concurrently
    pub async fn deploy_services(
        &mut self,
        clients: &Clients,
        component_sources: &ComponentSources,
    ) {
        let cosmos_trigger_code_map = CosmosTriggerCodeMap::new(DashMap::new());

        let mut futures = FuturesUnordered::new();

        for test in self.tests.iter_mut() {
            let clients = clients.clone();
            let component_sources = component_sources.clone();
            let cosmos_trigger_code_map = cosmos_trigger_code_map.clone();

            futures.push(async move {
                let (service, service_uri) = helpers::deploy_service_for_test(
                    test,
                    &clients,
                    &component_sources,
                    cosmos_trigger_code_map,
                )
                .await;

                for workflow in test.workflows.values() {
                    if let SubmitDefinition::Existing(Submit::Aggregator { url }) = &workflow.submit
                    {
                        TestRegistry::register_to_aggregator(url, &service, &service_uri)
                            .await
                            .unwrap();
                    }
                }

                test.service = Some(service);
            });
        }

        while (futures.next().await).is_some() {}
    }

    /// Registers a service on the aggregator
    pub async fn register_to_aggregator(
        aggregator_url: &str,
        service: &Service,
        service_uri: &str,
    ) -> Result<()> {
        let http_client = Client::new();

        let endpoint = format!("{}/register-service", aggregator_url);
        let payload = RegisterServiceRequest {
            uri: service_uri.to_string(),
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
    pub async fn from_test_mode(
        test_mode: crate::config::TestMode,
        chain_configs: &ChainConfigs,
        clients: &Clients,
    ) -> Self {
        // Convert TestMode to TestMatrix
        let matrix: TestMatrix = test_mode.into();

        // Get chain names
        let chain_names = ChainNames::from_config(chain_configs);

        let mut registry = Self::new();

        // Process EVM services
        for service in &matrix.evm {
            let chain = chain_names.primary_evm().unwrap();

            match service {
                EvmService::EchoData => {
                    registry.register_evm_echo_data_test(chain);
                }
                EvmService::EchoDataSecondaryChain => {
                    let secondary = chain_names.secondary_evm().unwrap();
                    registry.register_evm_echo_data_secondary_chain_test(secondary);
                }
                EvmService::EchoDataAggregator => {
                    registry.register_evm_echo_data_aggregator_test(chain, AGGREGATOR_ENDPOINT);
                }
                EvmService::Square => {
                    registry.register_evm_square_test(chain);
                }
                EvmService::ChainTriggerLookup => {
                    registry.register_evm_chain_trigger_lookup_test(chain);
                }
                EvmService::CosmosQuery => {
                    let cosmos = chain_names.primary_cosmos().unwrap();
                    registry.register_evm_cosmos_query_test(chain, cosmos);
                }
                EvmService::Permissions => {
                    registry.register_evm_permissions_test(chain);
                }
                EvmService::MultiWorkflow => {
                    registry.register_evm_multi_workflow_test(chain);
                }
                EvmService::MultiTrigger => {
                    let trigger = helpers::create_trigger_from_config(
                        TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: chain.clone(),
                            },
                        ),
                        clients,
                        CosmosTriggerCodeMap::new(DashMap::new()),
                        None,
                    )
                    .await;

                    registry.register_evm_multi_trigger_test(chain, trigger);
                }
                EvmService::BlockInterval => {
                    registry.register_evm_block_interval_test(chain);
                }
                EvmService::BlockIntervalStartStop => {
                    registry.register_evm_block_interval_start_stop_test(chain);
                }
                EvmService::CronInterval => {
                    registry.register_evm_cron_interval_test(chain);
                }
            }
        }

        // Process Cosmos services
        for service in &matrix.cosmos {
            let cosmos = chain_names.primary_cosmos().unwrap();
            let evm = chain_names.primary_evm().unwrap();

            match service {
                CosmosService::EchoData => {
                    registry.register_cosmos_echo_data_test(cosmos, evm);
                }
                CosmosService::Square => {
                    registry.register_cosmos_square_test(cosmos, evm);
                }
                CosmosService::ChainTriggerLookup => {
                    registry.register_cosmos_chain_trigger_lookup_test(cosmos, evm);
                }
                CosmosService::CosmosQuery => {
                    registry.register_cosmos_cosmos_query_test(cosmos, evm);
                }
                CosmosService::Permissions => {
                    registry.register_cosmos_permissions_test(cosmos, evm);
                }
                CosmosService::BlockInterval => {
                    registry.register_cosmos_block_interval_test(cosmos, evm);
                }
                CosmosService::BlockIntervalStartStop => {
                    registry.register_cosmos_block_interval_start_stop_test(cosmos, evm);
                }
                CosmosService::CronInterval => {
                    registry.register_cosmos_cron_interval_test(cosmos, evm);
                }
            }
        }

        // Process Cross-Chain services
        for service in &matrix.cross_chain {
            let cosmos = chain_names.primary_cosmos().unwrap();
            let evm = chain_names.primary_evm().unwrap();

            match service {
                CrossChainService::CosmosToEvmEchoData => {
                    registry.register_cosmos_to_evm_echo_data_test(cosmos, evm);
                }
            }
        }

        registry
    }

    // Individual test registration methods
    fn register_evm_echo_data_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_echo_data")
                .with_description("Tests the EchoData component on the primary EVM chain")
                .add_workflow(
                    WorkflowID::new("echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoData)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::Text("The times".to_string()))
                        .with_expected_output(ExpectedOutput::Text("The times".to_string()))
                        .build(),
                )
                .build(),
        )
    }

    fn register_evm_echo_data_secondary_chain_test(
        &mut self,
        secondary_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("evm_echo_data_secondary_chain")
                .with_description("Tests the EchoData component on the secondary EVM chain")
                .add_workflow(
                    WorkflowID::new("echo_data_secondary").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoData)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: secondary_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: secondary_chain.clone(),
                        })
                        .with_input_data(InputData::Text("collapse".to_string()))
                        .with_expected_output(ExpectedOutput::Text("collapse".to_string()))
                        .build(),
                )
                .with_service_manager_chain(secondary_chain)
                .build(),
        )
    }

    fn register_evm_echo_data_aggregator_test(
        &mut self,
        aggregator_chain: &ChainName,
        url: &str,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("evm_echo_data_aggregator")
                .with_description("Tests the EchoData component using an aggregator")
                .add_workflow(
                    WorkflowID::new("echo_data_aggregator").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoData)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: aggregator_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::Existing(Submit::Aggregator {
                            url: url.to_string(),
                        }))
                        .with_aggregator(AggregatorDefinition::NewEvmAggregatorSubmit {
                            chain_name: aggregator_chain.clone(),
                        })
                        .with_input_data(InputData::Text("Chancellor".to_string()))
                        .with_expected_output(ExpectedOutput::Text("Chancellor".to_string()))
                        .build(),
                )
                .with_service_manager_chain(aggregator_chain)
                .build(),
        )
    }

    fn register_evm_square_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_square")
                .with_description("Tests the Square component on EVM chain")
                .add_workflow(
                    WorkflowID::new("square").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::Square)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::Square { x: 3 })
                        .with_expected_output(ExpectedOutput::Square { y: 9 })
                        .build(),
                )
                .build(),
        )
    }

    fn register_evm_chain_trigger_lookup_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_chain_trigger_lookup")
                .with_description("Tests the ChainTriggerLookup component on EVM chain")
                .add_workflow(
                    WorkflowID::new("chain_trigger_lookup").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::ChainTriggerLookup)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::Text("satoshi".to_string()))
                        .with_expected_output(ExpectedOutput::Text("satoshi".to_string()))
                        .build(),
                )
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
                .with_description("Tests the CosmosQuery component from EVM to Cosmos")
                .add_workflow(
                    WorkflowID::new("cosmos_query").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::CosmosQuery)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: evm_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::CosmosQuery(CosmosQueryRequest::BlockHeight {
                            chain_name: cosmos_chain.clone(),
                        }))
                        .with_expected_output(ExpectedOutput::StructureOnly(
                            OutputStructure::CosmosQueryResponse,
                        ))
                        .build(),
                )
                .build(),
        )
    }

    fn register_evm_permissions_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_permissions")
                .with_description("Tests permissions for HTTP and file system access on EVM chain")
                .add_workflow(
                    WorkflowID::new("permissions").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::Permissions)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::Permissions(create_permissions_request()))
                        .with_expected_output(ExpectedOutput::StructureOnly(
                            OutputStructure::PermissionsResponse,
                        ))
                        .build(),
                )
                .build(),
        )
    }

    fn register_evm_multi_workflow_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_multi_workflow")
                .with_description("Tests multiple workflows in a single service on EVM chain")
                .add_workflow(
                    WorkflowID::new("square_workflow").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::Square)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::Square { x: 10 })
                        .with_expected_output(ExpectedOutput::Square { y: 100 })
                        .build(),
                )
                .add_workflow(
                    WorkflowID::new("echo_data_workflow").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoData)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain_name: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::Text("Multi-workflow".to_string()))
                        .with_expected_output(ExpectedOutput::Text("Multi-workflow".to_string()))
                        .build(),
                )
                .build(),
        )
    }

    fn register_evm_multi_trigger_test(
        &mut self,
        chain: &ChainName,
        trigger: Trigger,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("evm_multi_trigger")
                .with_description(
                    "Tests multiple services triggered by the same event on EVM chain",
                )
                .add_workflow(
                    WorkflowID::new("evm_multi_trigger").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoData)
                        .with_trigger(TriggerDefinition::Existing(trigger.clone()))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::Text("tttrrrrriiiigggeerrr".to_string()))
                        .with_expected_output(ExpectedOutput::Text(
                            "tttrrrrriiiigggeerrr".to_string(),
                        ))
                        .build(),
                )
                .add_workflow(
                    WorkflowID::new("evm_multi_trigger_2").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoData)
                        .with_trigger(TriggerDefinition::Existing(trigger))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::Text("tttrrrrriiiigggeerrr".to_string()))
                        .with_expected_output(ExpectedOutput::Text(
                            "tttrrrrriiiigggeerrr".to_string(),
                        ))
                        .build(),
                )
                .build(),
        )
    }

    fn register_evm_block_interval_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_block_interval")
                .with_description("Tests the block interval trigger on EVM chain")
                .add_workflow(
                    WorkflowID::new("block_interval").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoBlockInterval)
                        .with_trigger(TriggerDefinition::Existing(Trigger::BlockInterval {
                            chain_name: chain.clone(),
                            n_blocks: NonZeroU32::new(1).unwrap(),
                            start_block: None,
                            end_block: None,
                        }))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::None)
                        .with_expected_output(ExpectedOutput::Regex(
                            Regex::new(&format!("^{}", regex::escape(BLOCK_INTERVAL_DATA_PREFIX)))
                                .unwrap(),
                        ))
                        .build(),
                )
                .build(),
        )
    }

    fn register_evm_block_interval_start_stop_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_block_interval_start_stop")
                .with_description(
                    "Tests the block interval trigger with start/stop on an EVM chain",
                )
                .add_workflow(
                    WorkflowID::new("evm_block_interval_start_stop").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoBlockInterval)
                        .with_trigger(TriggerDefinition::DeferredBlockIntervalTarget {
                            chain_name: chain.clone(),
                        })
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::None)
                        .with_expected_output(ExpectedOutput::Deferred)
                        .build(),
                )
                .with_group(0)
                .build(),
        )
    }

    fn register_evm_cron_interval_test(&mut self, chain: &ChainName) -> &mut Self {
        self.register(
            TestBuilder::new("evm_cron_interval")
                .with_description("Tests the cron interval trigger")
                .add_workflow(
                    WorkflowID::new("cron_interval").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoCronInterval)
                        .with_trigger(TriggerDefinition::Existing(Trigger::Cron {
                            schedule: "* * * * * *".to_string(),
                            start_time: None,
                            end_time: None,
                        }))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: chain.clone(),
                        })
                        .with_input_data(InputData::None)
                        .with_expected_output(ExpectedOutput::Text(CRON_INTERVAL_DATA.to_owned()))
                        .build(),
                )
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
                .with_description("Tests the EchoData component on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoData)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain_name: cosmos_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::Text("on brink".to_string()))
                        .with_expected_output(ExpectedOutput::Text("on brink".to_string()))
                        .build(),
                )
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
                .with_description("Tests the Square component on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_square").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::Square)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain_name: cosmos_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::Square { x: 3 })
                        .with_expected_output(ExpectedOutput::Square { y: 9 })
                        .build(),
                )
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
                .with_description("Tests the ChainTriggerLookup component on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_chain_trigger_lookup").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::ChainTriggerLookup)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain_name: cosmos_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::Text("nakamoto".to_string()))
                        .with_expected_output(ExpectedOutput::Text("nakamoto".to_string()))
                        .build(),
                )
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
                .with_description("Tests the CosmosQuery component on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_cosmos_query").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::CosmosQuery)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain_name: cosmos_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::CosmosQuery(CosmosQueryRequest::BlockHeight {
                            chain_name: cosmos_chain.clone(),
                        }))
                        .with_expected_output(ExpectedOutput::StructureOnly(
                            OutputStructure::CosmosQueryResponse,
                        ))
                        .build(),
                )
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
                .with_description(
                    "Tests permissions for HTTP and file system access on Cosmos chain",
                )
                .add_workflow(
                    WorkflowID::new("cosmos_permissions").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::Permissions)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain_name: cosmos_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::Permissions(create_permissions_request()))
                        .with_expected_output(ExpectedOutput::StructureOnly(
                            OutputStructure::PermissionsResponse,
                        ))
                        .build(),
                )
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
                .with_description("Tests the block interval trigger on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_block_interval").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoBlockInterval)
                        .with_trigger(TriggerDefinition::Existing(Trigger::BlockInterval {
                            chain_name: cosmos_chain.clone(),
                            n_blocks: NonZeroU32::new(1).unwrap(),
                            start_block: None,
                            end_block: None,
                        }))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::None)
                        .with_expected_output(ExpectedOutput::Regex(
                            Regex::new(&format!("^{}", regex::escape(BLOCK_INTERVAL_DATA_PREFIX)))
                                .unwrap(),
                        ))
                        .build(),
                )
                .build(),
        )
    }

    fn register_cosmos_block_interval_start_stop_test(
        &mut self,
        cosmos_chain: &ChainName,
        evm_chain: &ChainName,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_block_interval_start_stop")
                .with_description(
                    "Tests the block interval trigger with start/stop on a Cosmos chain",
                )
                .add_workflow(
                    WorkflowID::new("cosmos_block_interval_start_stop").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoBlockInterval)
                        .with_trigger(TriggerDefinition::DeferredBlockIntervalTarget {
                            chain_name: cosmos_chain.clone(),
                        })
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::None)
                        .with_expected_output(ExpectedOutput::Deferred)
                        .build(),
                )
                .with_group(0)
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
                .with_description("Tests the cron interval trigger on Cosmos chain")
                .add_workflow(
                    WorkflowID::new("cosmos_cron_interval").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoCronInterval)
                        .with_trigger(TriggerDefinition::Existing(Trigger::Cron {
                            schedule: "* * * * * *".to_string(),
                            start_time: None,
                            end_time: None,
                        }))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::None)
                        .with_expected_output(ExpectedOutput::Text(CRON_INTERVAL_DATA.to_owned()))
                        .build(),
                )
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
                .with_description("Tests the EchoData component from Cosmos to EVM")
                .add_workflow(
                    WorkflowID::new("cross_chain_echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .with_component(ComponentName::EchoData)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain_name: cosmos_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::NewEvmContract {
                            chain_name: evm_chain.clone(),
                        })
                        .with_input_data(InputData::Text("hello EVM world from cosmos".to_string()))
                        .with_expected_output(ExpectedOutput::Text(
                            "hello EVM world from cosmos".to_string(),
                        ))
                        .build(),
                )
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
