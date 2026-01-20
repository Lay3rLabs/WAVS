use dashmap::DashMap;
use example_types::{
    BlockIntervalResponse, CosmosQueryRequest, KvStoreRequest, KvStoreResponse, PermissionsRequest,
    PermissionsResponse, SquareRequest, SquareResponse,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use wavs_types::AtProtoAction;

use super::clients::Clients;
use super::components::{AggregatorComponent, ComponentName, OperatorComponent};
use super::config::CRON_INTERVAL_DATA;
use super::matrix::{CosmosService, CrossChainService, EvmService, TestMatrix};
use super::test_definition::{
    AggregatorDefinition, CosmosTriggerDefinition, EvmTriggerDefinition, ExpectedOutput, InputData,
    OutputStructure, SubmitDefinition, TestBuilder, TestDefinition, TriggerDefinition,
    WorkflowBuilder,
};
use crate::e2e::chains::ChainKeys;
use crate::e2e::components::ComponentSources;
use crate::e2e::helpers::create_trigger_from_config;
use crate::e2e::test_definition::{
    ChangeServiceDefinition, ComponentDefinition, CosmosSubmitDefinition, ExpectedOutputCallback,
    TestGroupId,
};
use wavs_types::{ChainConfigs, ChainKey, Trigger, WorkflowId};

/// This map is used to ensure cosmos contracts only have their wasm uploaded once
/// Key -> Cosmos Trigger Definition, Value -> Maybe Code Id
pub type CosmosCodeMap =
    Arc<DashMap<CosmosContractDefinition, Arc<tokio::sync::RwLock<Option<u64>>>>>;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CosmosContractDefinition {
    Trigger(CosmosTriggerDefinition),
    Submit(CosmosSubmitDefinition),
}

use super::handles::hypercore::HypercoreTestClient;

/// Registry for managing test definitions and their deployed services
pub struct TestRegistry {
    tests: Vec<TestDefinition>,
    /// Map of test name to hypercore test client for real hypercore e2e tests
    hypercore_clients: DashMap<String, Arc<HypercoreTestClient>>,
}

impl Default for TestRegistry {
    fn default() -> Self {
        Self {
            tests: Vec::new(),
            hypercore_clients: DashMap::new(),
        }
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
        self.tests.push(test);
        self
    }

    /// Group all test definitions by group (ascending priority)
    pub fn list_all_grouped(
        &self,
        allow_grouping: bool,
    ) -> BTreeMap<TestGroupId, Vec<TestDefinition>> {
        let mut map: BTreeMap<TestGroupId, Vec<TestDefinition>> = BTreeMap::new();

        for (index, test) in self.tests.iter().cloned().enumerate() {
            if allow_grouping {
                map.entry(test.group).or_default().push(test);
            } else {
                map.entry(TestGroupId::Other(index)).or_default().push(test);
            }
        }
        map
    }

    pub fn list_all(&self) -> impl Iterator<Item = &TestDefinition> {
        self.tests.iter()
    }

    /// Get a hypercore test client by test name
    pub fn get_hypercore_client(&self, test_name: &str) -> Option<Arc<HypercoreTestClient>> {
        self.hypercore_clients.get(test_name).map(|v| v.clone())
    }

    /// Store a hypercore test client for a test
    pub fn insert_hypercore_client(&self, test_name: String, client: HypercoreTestClient) {
        self.hypercore_clients.insert(test_name, Arc::new(client));
    }

    /// Create a registry based on the test mode
    pub async fn from_test_mode(
        test_mode: crate::config::TestMode,
        chain_configs: Arc<RwLock<ChainConfigs>>,
        clients: &Clients,
        cosmos_code_map: &CosmosCodeMap,
        hyperswarm_bootstrap: Option<String>,
    ) -> Self {
        // Convert TestMode to TestMatrix
        let matrix: TestMatrix = test_mode.into();

        // Get chain names
        let chains = ChainKeys::from_config(&chain_configs.read().unwrap());

        let mut registry = Self::new();

        // Process EVM services
        for service in &matrix.evm {
            let chain = chains.primary_evm().unwrap();

            match service {
                EvmService::EchoData => {
                    registry.register_evm_echo_data_test(chain);
                }
                EvmService::AtprotoEchoData => {
                    registry.register_evm_atproto_echo_data_test(chain);
                }
                EvmService::HypercoreEchoData => {
                    registry
                        .register_evm_hypercore_echo_data_test(chain, hyperswarm_bootstrap.clone())
                        .await;
                }
                EvmService::EchoDataSecondaryChain => {
                    let secondary = chains.secondary_evm().unwrap();
                    registry.register_evm_echo_data_secondary_chain_test(secondary);
                }
                EvmService::Square => {
                    registry.register_evm_square_test(chain);
                }
                EvmService::ChainTriggerLookup => {
                    registry.register_evm_chain_trigger_lookup_test(chain);
                }
                EvmService::CosmosQuery => {
                    let cosmos = chains.primary_cosmos().unwrap();
                    registry.register_evm_cosmos_query_test(chain, cosmos);
                }
                EvmService::KvStore => {
                    registry.register_evm_kv_store_test(chain);
                }
                EvmService::Permissions => {
                    registry.register_evm_permissions_test(chain);
                }
                EvmService::MultiWorkflow => {
                    registry.register_evm_multi_workflow_test(chain);
                }
                EvmService::ChangeWorkflow => {
                    registry.register_evm_change_workflow_test(chain);
                }
                EvmService::MultiTrigger => {
                    let trigger = create_trigger_from_config(
                        TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ),
                        clients,
                        cosmos_code_map.clone(),
                        None,
                    )
                    .await;

                    registry.register_evm_multi_trigger_test(chain, trigger);
                }
                EvmService::TriggerBackpressure => {
                    registry.register_evm_trigger_backpressure_test(chain);
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
                EvmService::EmptyToEchoData => {
                    registry.register_evm_empty_to_echo_data_test(chain);
                }
                EvmService::SimpleAggregator => {
                    registry.register_evm_simple_aggregator_test(chain);
                }
                EvmService::TimerAggregator => {
                    registry.register_evm_timer_aggregator_test(chain);
                }
                EvmService::TimerAggregatorReorg => {
                    registry.register_evm_timer_aggregator_reorg_test(chain);
                }
                EvmService::GasPrice => {
                    registry.register_evm_gas_price_test(chain);
                }
                EvmService::MultiOperator => {
                    registry.register_evm_multi_operator_test(chain);
                }
            }
        }

        // Process Cosmos services
        for service in &matrix.cosmos {
            let cosmos = chains.primary_cosmos().unwrap();

            match service {
                CosmosService::EchoData => {
                    registry.register_cosmos_echo_data_test(cosmos, cosmos);
                }
                CosmosService::Square => {
                    registry.register_cosmos_square_test(cosmos, cosmos);
                }
                CosmosService::ChainTriggerLookup => {
                    registry.register_cosmos_chain_trigger_lookup_test(cosmos, cosmos);
                }
                CosmosService::CosmosQuery => {
                    registry.register_cosmos_cosmos_query_test(cosmos, cosmos);
                }
                CosmosService::Permissions => {
                    registry.register_cosmos_permissions_test(cosmos, cosmos);
                }
                CosmosService::BlockInterval => {
                    registry.register_cosmos_block_interval_test(cosmos, cosmos);
                }
                CosmosService::BlockIntervalStartStop => {
                    registry.register_cosmos_block_interval_start_stop_test(cosmos, cosmos);
                }
                CosmosService::CronInterval => {
                    registry.register_cosmos_cron_interval_test(cosmos, cosmos);
                }
            }
        }

        // Process Cross-Chain services
        for service in &matrix.cross_chain {
            let cosmos = chains.primary_cosmos().unwrap();
            let evm = chains.primary_evm().unwrap();

            match service {
                CrossChainService::CosmosToEvmEchoData => {
                    registry.register_cosmos_to_evm_echo_data_test(cosmos, evm);
                }
            }
        }

        registry
    }

    // Helper function to create simple aggregator configuration
    fn simple_aggregator(chain: &ChainKey) -> AggregatorDefinition {
        AggregatorDefinition::ComponentBasedAggregator {
            component: ComponentDefinition::from(ComponentName::Aggregator(
                AggregatorComponent::SimpleAggregator,
            ))
            .with_config_hardcoded("chain".to_string(), chain.to_string())
            .with_config_service_handler(),
            chain: chain.clone(),
        }
    }

    // Individual test registration methods
    fn register_evm_echo_data_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_echo_data")
                .with_description("Tests the EchoData component on the primary EVM chain")
                .add_workflow(
                    WorkflowId::new("echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_input_data(InputData::Text("The times".to_string()))
                        .with_expected_output(ExpectedOutput::Text("The times".to_string()))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_atproto_echo_data_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_atproto_echo_data")
                .with_description("Tests the EchoData component handling ATProto triggers")
                .add_workflow(
                    WorkflowId::new("atproto_echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::Existing(Trigger::AtProtoEvent {
                            collection: "app.bsky.feed.post".to_string(),
                            repo_did: Some("did:example:alice".to_string()),
                            action: Some(AtProtoAction::Create),
                        }))
                        .with_input_data(InputData::Text("atproto-echo".to_string()))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Text(
                            json!({"text": "atproto-echo"}).to_string(),
                        ))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    async fn register_evm_hypercore_echo_data_test(
        &mut self,
        chain: &ChainKey,
        hyperswarm_bootstrap: Option<String>,
    ) -> &mut Self {
        // Create a real hypercore test client with generated feed key
        let test_name = "evm_hypercore_echo_data";
        let hypercore_client = HypercoreTestClient::new(test_name, hyperswarm_bootstrap)
            .await
            .expect("Failed to create hypercore test client");

        let feed_key = hypercore_client.feed_key();

        tracing::info!(
            "Created hypercore test client for '{}' with feed key: {}",
            test_name,
            feed_key
        );

        // Store the client for use during test execution
        self.insert_hypercore_client(test_name.to_string(), hypercore_client);

        self.register(
            TestBuilder::new(test_name)
                .with_description(
                    "Tests the EchoData component with real Hypercore append triggers",
                )
                .add_workflow(
                    WorkflowId::new("hypercore_echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::Existing(Trigger::HypercoreAppend {
                            feed_key,
                        }))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_input_data(InputData::Text("hypercore-echo".to_string()))
                        .with_expected_output(ExpectedOutput::Text("hypercore-echo".to_string()))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .with_group(TestGroupId::Hypercore)
                .build(),
        )
    }

    fn register_evm_echo_data_secondary_chain_test(
        &mut self,
        secondary_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("evm_echo_data_secondary_chain")
                .with_description("Tests the EchoData component on the secondary EVM chain")
                .add_workflow(
                    WorkflowId::new("echo_data_secondary").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: secondary_chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("collapse".to_string()))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            secondary_chain,
                        )))
                        .with_expected_output(ExpectedOutput::Text("collapse".to_string()))
                        .build(),
                )
                .with_service_manager_chain(secondary_chain)
                .build(),
        )
    }

    fn register_evm_empty_to_echo_data_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_empty_to_echo_data")
                .with_description("Tests going from empty service workflows to some")
                .with_service_manager_chain(chain)
                .with_change_service(ChangeServiceDefinition::AddWorkflow {
                    workflow_id: WorkflowId::new("echo_data").unwrap(),
                    workflow: WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("The times".to_string()))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Text("The times".to_string()))
                        .build(),
                })
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_simple_aggregator_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_simple_aggregator")
                .with_description("Tests the SimpleAggregator component-based aggregation")
                .add_workflow(
                    WorkflowId::new("simple_aggregator").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::Aggregator(
                            AggregatorDefinition::ComponentBasedAggregator {
                                component: ComponentDefinition::from(ComponentName::Aggregator(
                                    AggregatorComponent::SimpleAggregator,
                                ))
                                .with_config_hardcoded("chain".to_string(), chain.to_string())
                                .with_config_service_handler(),
                                // for deploying the submission contract that the aggregator will use
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("test packet".to_string()))
                        .with_expected_output(ExpectedOutput::Text("test packet".to_string()))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_timer_aggregator_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_timer_aggregator")
                .with_description("Tests the TimerAggregator component with delayed submission")
                .add_workflow(
                    WorkflowId::new("timer_aggregator").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::TimerAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::Aggregator(
                            AggregatorDefinition::ComponentBasedAggregator {
                                component: ComponentDefinition::from(ComponentName::Aggregator(
                                    AggregatorComponent::TimerAggregator,
                                ))
                                .with_config_hardcoded("chain".to_string(), chain.to_string())
                                .with_config_hardcoded(
                                    "timer_delay_secs".to_string(),
                                    "3".to_string(),
                                )
                                .with_config_service_handler(),
                                // for deploying the submission contract that the aggregator will use
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("test packet".to_string()))
                        .with_expected_output(ExpectedOutput::Text("test packet".to_string()))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .with_group(TestGroupId::AggregatorTimer)
                .build(),
        )
    }

    fn register_evm_timer_aggregator_reorg_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_timer_aggregator_reorg")
                .with_description("Tests TimerAggregator component with delayed submission and re-org handling - expected output should be dropped")
                .add_workflow(
                    WorkflowId::new("timer_aggregator_reorg").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::TimerAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::Aggregator(
                            AggregatorDefinition::ComponentBasedAggregator {
                                component: ComponentDefinition::from(ComponentName::Aggregator(
                                    AggregatorComponent::TimerAggregator,
                                ))
                                .with_config_hardcoded("chain".to_string(), chain.to_string())
                                .with_config_hardcoded(
                                    "timer_delay_secs".to_string(),
                                    "3".to_string(),
                                )
                                .with_config_service_handler(),
                                // for deploying the submission contract that the aggregator will use
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("reorg test packet".to_string()))
                        .with_expected_output(ExpectedOutput::Dropped)
                        .build(),
                )
                .with_service_manager_chain(chain)
                .with_group(TestGroupId::AggregatorTimer)
                .build(),
        )
    }

    fn register_evm_gas_price_test(&mut self, chain: &ChainKey) -> &mut Self {
        // Only run this test if ETHERSCAN_API_KEY is set
        let api_key = std::env::var("ETHERSCAN_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            tracing::warn!("Skipping gas price test - ETHERSCAN_API_KEY not set");
            return self;
        }

        self.register(
            TestBuilder::new("evm_gas_price")
                .with_description("Tests gas price fetching from Etherscan API")
                .add_workflow(
                    WorkflowId::new("gas_price_test").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::Aggregator(
                            AggregatorDefinition::ComponentBasedAggregator {
                                component: ComponentDefinition::from(ComponentName::Aggregator(
                                    AggregatorComponent::SimpleAggregator,
                                ))
                                .with_config_hardcoded("chain".to_string(), chain.to_string())
                                .with_env_var("ETHERSCAN_API_KEY".to_string(), api_key)
                                .with_config_hardcoded(
                                    "gas_strategy".to_string(),
                                    "standard".to_string(),
                                )
                                .with_config_service_handler(),
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("gas test".to_string()))
                        .with_expected_output(ExpectedOutput::Text("gas test".to_string()))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_square_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_square")
                .with_description("Tests the Square component on EVM chain")
                .add_workflow(
                    WorkflowId::new("square").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::Square)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Square(SquareRequest { x: 3 }))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Square(SquareResponse { y: 9 }))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_chain_trigger_lookup_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_chain_trigger_lookup")
                .with_description("Tests the ChainTriggerLookup component on EVM chain")
                .add_workflow(
                    WorkflowId::new("chain_trigger_lookup").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::ChainTriggerLookup)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("satoshi".to_string()))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Text("satoshi".to_string()))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_cosmos_query_test(
        &mut self,
        evm_chain: &ChainKey,
        cosmos_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("evm_cosmos_query")
                .with_description("Tests the CosmosQuery component from EVM to Cosmos")
                .add_workflow(
                    WorkflowId::new("cosmos_query").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::CosmosQuery)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: evm_chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::CosmosQuery(CosmosQueryRequest::BlockHeight {
                            chain: cosmos_chain.to_string(),
                        }))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            evm_chain,
                        )))
                        .with_expected_output(ExpectedOutput::StructureOnly(
                            OutputStructure::CosmosQueryResponse,
                        ))
                        .build(),
                )
                .with_service_manager_chain(evm_chain)
                .build(),
        )
    }

    fn register_evm_permissions_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_permissions")
                .with_description("Tests permissions for HTTP and file system access on EVM chain")
                .add_workflow(
                    WorkflowId::new("permissions").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::Permissions)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Permissions(create_permissions_request()))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Callback(PermissionsCallback::new()))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_kv_store_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_kv_store")
                .with_description(
                    "Tests counter component running twice to verify keyvalue persistence",
                )
                .add_workflow(
                    WorkflowId::new("counter_first").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::KvStore)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::KvStore(KvStoreRequest::Write {
                            bucket: "test_bucket".to_string(),
                            key: "hello".to_string(),
                            value: b"world".to_vec(),
                        }))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::KvStore(KvStoreResponse::Write))
                        .build(),
                )
                .add_workflow(
                    WorkflowId::new("counter_second").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::KvStore)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::KvStore(KvStoreRequest::Read {
                            bucket: "test_bucket".to_string(),
                            key: "hello".to_string(),
                        }))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::KvStore(KvStoreResponse::Read {
                            value: b"world".to_vec(),
                        }))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_multi_workflow_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_multi_workflow")
                .with_description("Tests multiple workflows with different components on EVM chain")
                .add_workflow(
                    WorkflowId::new("square_workflow").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::Square)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Square(SquareRequest { x: 5 }))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Square(SquareResponse { y: 25 }))
                        .build(),
                )
                .add_workflow(
                    WorkflowId::new("echo_workflow").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("hello workflows".to_string()))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Text("hello workflows".to_string()))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_change_workflow_test(&mut self, chain: &ChainKey) -> &mut Self {
        let workflow_id = WorkflowId::new("change_workflow").unwrap();

        self.register(
            TestBuilder::new("evm_change_workflow")
                .with_description("Tests changing workflows in a single service on EVM chain")
                .add_workflow(
                    workflow_id.clone(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::Square)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Square(SquareRequest { x: 10 }))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        // the original component is square, and so we expect '{"y": 100}'
                        // but when we swap the component, we just get the original trigger echoed back
                        .with_expected_output(ExpectedOutput::EchoSquare { x: 10 })
                        .build(),
                )
                .with_change_service(ChangeServiceDefinition::Component {
                    workflow_id,
                    component: ComponentName::Operator(OperatorComponent::EchoData).into(),
                })
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_multi_trigger_test(&mut self, chain: &ChainKey, trigger: Trigger) -> &mut Self {
        self.register(
            TestBuilder::new("evm_multi_trigger")
                .with_description(
                    "Tests multiple services triggered by the same event on EVM chain",
                )
                .add_workflow(
                    WorkflowId::new("evm_multi_trigger").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::Existing(trigger.clone()))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_input_data(InputData::Text("tttrrrrriiiigggeerrr".to_string()))
                        .with_expected_output(ExpectedOutput::Text(
                            "tttrrrrriiiigggeerrr".to_string(),
                        ))
                        .build(),
                )
                .add_workflow(
                    WorkflowId::new("evm_multi_trigger_2").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::Existing(trigger))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_input_data(InputData::Text("tttrrrrriiiigggeerrr".to_string()))
                        .with_expected_output(ExpectedOutput::Text(
                            "tttrrrrriiiigggeerrr".to_string(),
                        ))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .build(),
        )
    }

    fn register_evm_trigger_backpressure_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_trigger_backpressure")
                .with_description("Floods trigger logs to expose the subscribe_logs buffer limit")
                .add_workflow(
                    WorkflowId::new("trigger_backpressure").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("trigger-backpressure".to_string()))
                        .with_log_spam_count(64)
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Text(
                            "trigger-backpressure".to_string(),
                        ))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .with_group(TestGroupId::Backpressure)
                .build(),
        )
    }

    fn register_evm_block_interval_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_block_interval")
                .with_description("Tests the block interval trigger on EVM chain")
                .add_workflow(
                    WorkflowId::new("block_interval").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoBlockInterval)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::BlockInterval {
                            chain: chain.clone(),
                            start_stop: false,
                        })
                        .with_input_data(InputData::None)
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Callback(BlockIntervalCallback::new(
                            false,
                        )))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .with_group(TestGroupId::EvmInterval)
                .build(),
        )
    }

    fn register_evm_block_interval_start_stop_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_block_interval_start_stop")
                .with_description(
                    "Tests the block interval trigger with start/stop on an EVM chain",
                )
                .add_workflow(
                    WorkflowId::new("evm_block_interval_start_stop").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoBlockInterval)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::BlockInterval {
                            chain: chain.clone(),
                            start_stop: true,
                        })
                        .with_input_data(InputData::None)
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Callback(BlockIntervalCallback::new(
                            true,
                        )))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .with_group(TestGroupId::EvmIntervalStartStop)
                .build(),
        )
    }

    fn register_evm_cron_interval_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_cron_interval")
                .with_description("Tests the cron interval trigger")
                .add_workflow(
                    WorkflowId::new("cron_interval").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoCronInterval)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::Existing(Trigger::Cron {
                            schedule: "*/5 * * * * *".to_string(),
                            start_time: None,
                            end_time: None,
                        }))
                        .with_input_data(InputData::None)
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_expected_output(ExpectedOutput::Text(CRON_INTERVAL_DATA.to_owned()))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .with_group(TestGroupId::CronInterval)
                .build(),
        )
    }

    /// Multi-operator test (P2P mode configured via layer-tests.toml)
    fn register_evm_multi_operator_test(&mut self, chain: &ChainKey) -> &mut Self {
        self.register(
            TestBuilder::new("evm_multi_operator")
                .with_description("Tests multi-operator quorum (2/3) with P2P networking")
                .add_workflow(
                    WorkflowId::new("multi_operator_echo").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewEvmContract(
                            EvmTriggerDefinition::SimpleContractEvent {
                                chain: chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(chain)))
                        .with_input_data(InputData::Text("multi-operator test".to_string()))
                        .with_expected_output(ExpectedOutput::Text(
                            "multi-operator test".to_string(),
                        ))
                        .build(),
                )
                .with_service_manager_chain(chain)
                .with_multi_operator()
                .with_group(TestGroupId::P2p)
                .build(),
        )
    }

    // Cosmos test registrations

    fn register_cosmos_echo_data_test(
        &mut self,
        trigger_chain: &ChainKey,
        submit_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_echo_data")
                .with_description("Tests the EchoData component on Cosmos chain")
                .add_workflow(
                    WorkflowId::new("cosmos_echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain: trigger_chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("on brink".to_string()))
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            submit_chain,
                        )))
                        .with_expected_output(ExpectedOutput::Text("on brink".to_string()))
                        .build(),
                )
                .with_service_manager_chain(submit_chain)
                .build(),
        )
    }

    fn register_cosmos_square_test(
        &mut self,
        trigger_chain: &ChainKey,
        submit_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_square")
                .with_description("Tests the Square component on Cosmos chain")
                .add_workflow(
                    WorkflowId::new("cosmos_square").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::Square)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain: trigger_chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Square(SquareRequest { x: 3 }))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            submit_chain,
                        )))
                        .with_expected_output(ExpectedOutput::Square(SquareResponse { y: 9 }))
                        .build(),
                )
                .with_service_manager_chain(submit_chain)
                .build(),
        )
    }

    fn register_cosmos_chain_trigger_lookup_test(
        &mut self,
        trigger_chain: &ChainKey,
        submit_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_chain_trigger_lookup")
                .with_description("Tests the ChainTriggerLookup component on Cosmos chain")
                .add_workflow(
                    WorkflowId::new("cosmos_chain_trigger_lookup").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::ChainTriggerLookup)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain: trigger_chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Text("nakamoto".to_string()))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            submit_chain,
                        )))
                        .with_expected_output(ExpectedOutput::Text("nakamoto".to_string()))
                        .build(),
                )
                .with_service_manager_chain(submit_chain)
                .build(),
        )
    }

    fn register_cosmos_cosmos_query_test(
        &mut self,
        trigger_chain: &ChainKey,
        submit_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_cosmos_query")
                .with_description("Tests the CosmosQuery component on Cosmos chain")
                .add_workflow(
                    WorkflowId::new("cosmos_cosmos_query").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::CosmosQuery)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain: trigger_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            submit_chain,
                        )))
                        .with_input_data(InputData::CosmosQuery(CosmosQueryRequest::BlockHeight {
                            chain: trigger_chain.to_string(),
                        }))
                        .with_expected_output(ExpectedOutput::StructureOnly(
                            OutputStructure::CosmosQueryResponse,
                        ))
                        .build(),
                )
                .with_service_manager_chain(submit_chain)
                .build(),
        )
    }

    fn register_cosmos_permissions_test(
        &mut self,
        trigger_chain: &ChainKey,
        submit_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_permissions")
                .with_description(
                    "Tests permissions for HTTP and file system access on Cosmos chain",
                )
                .add_workflow(
                    WorkflowId::new("cosmos_permissions").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::Permissions)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain: trigger_chain.clone(),
                            },
                        ))
                        .with_input_data(InputData::Permissions(create_permissions_request()))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            submit_chain,
                        )))
                        .with_expected_output(ExpectedOutput::StructureOnly(
                            OutputStructure::PermissionsResponse,
                        ))
                        .build(),
                )
                .with_service_manager_chain(submit_chain)
                .build(),
        )
    }

    fn register_cosmos_block_interval_test(
        &mut self,
        trigger_chain: &ChainKey,
        submit_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_block_interval")
                .with_description("Tests the block interval trigger on Cosmos chain")
                .add_workflow(
                    WorkflowId::new("cosmos_block_interval").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoBlockInterval)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::BlockInterval {
                            chain: trigger_chain.clone(),
                            start_stop: false,
                        })
                        .with_input_data(InputData::None)
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            submit_chain,
                        )))
                        .with_expected_output(ExpectedOutput::Callback(BlockIntervalCallback::new(
                            false,
                        )))
                        .build(),
                )
                .with_service_manager_chain(submit_chain)
                .with_group(TestGroupId::CosmosInterval)
                .build(),
        )
    }

    fn register_cosmos_block_interval_start_stop_test(
        &mut self,
        trigger_chain: &ChainKey,
        submit_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_block_interval_start_stop")
                .with_description(
                    "Tests the block interval trigger with start/stop on a Cosmos chain",
                )
                .add_workflow(
                    WorkflowId::new("cosmos_block_interval_start_stop").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoBlockInterval)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::BlockInterval {
                            chain: trigger_chain.clone(),
                            start_stop: true,
                        })
                        .with_input_data(InputData::None)
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            submit_chain,
                        )))
                        .with_expected_output(ExpectedOutput::Callback(BlockIntervalCallback::new(
                            true,
                        )))
                        .build(),
                )
                .with_service_manager_chain(submit_chain)
                .with_group(TestGroupId::CosmosIntervalStartStop)
                .build(),
        )
    }

    fn register_cosmos_cron_interval_test(
        &mut self,
        _trigger_chain: &ChainKey,
        submit_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cosmos_cron_interval")
                .with_description("Tests the cron interval trigger on Cosmos chain")
                .add_workflow(
                    WorkflowId::new("cosmos_cron_interval").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoCronInterval)
                        .with_aggregator_component(AggregatorComponent::SimpleAggregator)
                        .with_trigger(TriggerDefinition::Existing(Trigger::Cron {
                            schedule: "*/5 * * * * *".to_string(),
                            start_time: None,
                            end_time: None,
                        }))
                        .with_input_data(InputData::None)
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            submit_chain,
                        )))
                        .with_expected_output(ExpectedOutput::Text(CRON_INTERVAL_DATA.to_owned()))
                        .build(),
                )
                .with_service_manager_chain(submit_chain)
                .with_group(TestGroupId::CronInterval)
                .build(),
        )
    }

    // Cross-chain test registrations

    fn register_cosmos_to_evm_echo_data_test(
        &mut self,
        cosmos_chain: &ChainKey,
        evm_chain: &ChainKey,
    ) -> &mut Self {
        self.register(
            TestBuilder::new("cross_chain_cosmos_to_evm_echo_data")
                .with_description("Tests the EchoData component from Cosmos to EVM")
                .add_workflow(
                    WorkflowId::new("cross_chain_echo_data").unwrap(),
                    WorkflowBuilder::new()
                        .with_operator_component(OperatorComponent::EchoData)
                        .with_trigger(TriggerDefinition::NewCosmosContract(
                            CosmosTriggerDefinition::SimpleContractEvent {
                                chain: cosmos_chain.clone(),
                            },
                        ))
                        .with_submit(SubmitDefinition::Aggregator(Self::simple_aggregator(
                            evm_chain,
                        )))
                        .with_input_data(InputData::Text("hello EVM world from cosmos".to_string()))
                        .with_expected_output(ExpectedOutput::Text(
                            "hello EVM world from cosmos".to_string(),
                        ))
                        .build(),
                )
                .with_service_manager_chain(evm_chain)
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

#[derive(Clone, Debug)]
struct BlockIntervalCallback {
    pub start_stop: bool,
}

impl BlockIntervalCallback {
    pub fn new(start_stop: bool) -> Arc<Self> {
        Arc::new(BlockIntervalCallback { start_stop })
    }
}

impl ExpectedOutputCallback for BlockIntervalCallback {
    fn validate(
        &self,
        test: &TestDefinition,
        _clients: &super::clients::Clients,
        _component_sources: &ComponentSources,
        actual: &[u8],
    ) -> anyhow::Result<()> {
        let response: BlockIntervalResponse = serde_json::from_slice(actual)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize block interval response: {}", e))?;

        if let Some(start) = response.trigger_config_start {
            tracing::info!(
                "[{}] count: {}, triggered at: {}, configured start at: {}",
                test.name,
                response.count,
                response.trigger_data_block_height,
                start
            );
            anyhow::ensure!(
                start <= response.trigger_data_block_height,
                "Start block must be less than or equal to trigger data block height"
            );
        } else {
            tracing::info!(
                "[{}] count: {}, triggered at: {}",
                test.name,
                response.count,
                response.trigger_data_block_height
            );
        }

        if self.start_stop {
            match (response.trigger_config_start, response.trigger_config_end) {
                (Some(start), Some(end)) => {
                    // Ensure the start and end are set correctly
                    anyhow::ensure!(
                        start == end,
                        "Start block must be exactly equal to end block"
                    );
                    anyhow::ensure!(
                        response.count == 1,
                        "Trigger should only be called exactly once for start/stop"
                    );
                }
                _ => {
                    return Err(anyhow::anyhow!(
                        "Expected both trigger_config_start and trigger_config_end to be set"
                    ));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct PermissionsCallback {}

impl PermissionsCallback {
    pub fn new() -> Arc<Self> {
        Arc::new(PermissionsCallback {})
    }
}

impl ExpectedOutputCallback for PermissionsCallback {
    fn validate(
        &self,
        _test: &TestDefinition,
        _clients: &super::clients::Clients,
        component_sources: &ComponentSources,
        actual: &[u8],
    ) -> anyhow::Result<()> {
        let response: PermissionsResponse = serde_json::from_slice(actual)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize permissions response: {}", e))?;

        let digest = component_sources
            .lookup
            .get(&ComponentName::Operator(OperatorComponent::Permissions))
            .ok_or_else(|| anyhow::anyhow!("Failed to get digest for Permissions component"))?
            .digest()
            .to_string();

        anyhow::ensure!(
            response.digest == digest,
            "Unexpected digest in permissions response"
        );
        Ok(())
    }
}
