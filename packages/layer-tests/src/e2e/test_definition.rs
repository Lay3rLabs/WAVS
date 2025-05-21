#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroU64};

use anyhow::{bail, ensure, Context};
use wavs_types::{Aggregator, ChainName, Service, Submit, Timestamp, Trigger, WorkflowID};

use crate::e2e::components::ComponentName;
use crate::e2e::types::{CosmosQueryRequest, PermissionsRequest, SquareRequest, SquareResponse};

use super::types::{CosmosQueryResponse, PermissionsResponse};

/// Defines a complete end-to-end test case
#[derive(Clone, Debug)]
pub struct TestDefinition {
    /// Unique name for this test
    pub name: String,

    /// Description of what this test verifies
    pub description: Option<String>,

    /// The workflows of this test
    pub workflows: BTreeMap<WorkflowID, WorkflowConfig>,

    /// Service manager chain
    pub service_manager_chain: ChainName,

    /// Reference to the deployed service (populated during test execution)
    pub service: Option<Service>,

    /// Run priority
    pub priority: u64,
}

impl PartialEq for TestDefinition {
    fn eq(&self, other: &Self) -> bool {
        self.priority.eq(&other.priority) && self.name.eq(&other.name)
    }
}

impl Eq for TestDefinition {}

impl PartialOrd for TestDefinition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TestDefinition {
    fn cmp(&self, other: &Self) -> Ordering {
        // First compare block interval
        let by_priority = self.priority.cmp(&other.priority);
        if by_priority != Ordering::Equal {
            return by_priority;
        }

        // Then use name as a stable tiebreaker
        self.name.cmp(&other.name)
    }
}

#[derive(Clone, Debug)]
pub struct WorkflowConfig {
    /// Components used in this test
    pub component: ComponentName,

    /// Trigger configuration
    pub trigger: TriggerDefinition,

    /// Submit configuration
    pub submit: SubmitDefinition,

    /// Aggregators configuration
    pub aggregators: Vec<AggregatorDefinition>,

    /// Input data to send to the trigger
    pub input_data: InputData,

    /// Expected output to verify
    pub expected_output: ExpectedOutput,
}

#[derive(Clone, Debug)]
pub enum AggregatorDefinition {
    NewEvmAggregatorSubmit { chain_name: ChainName },
    Aggregator(Aggregator),
}

/// Configuration for a trigger
#[derive(Clone, Debug)]
pub enum TriggerDefinition {
    Evm(EvmTriggerDefinition),

    Cosmos(CosmosTriggerDefinition),

    /// Use an existing trigger
    Trigger(Trigger),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CosmosTriggerDefinition {
    Simple { chain_name: ChainName },
}

#[derive(Clone, Debug)]
pub enum EvmTriggerDefinition {
    Simple { chain_name: ChainName },
}

/// Configuration for a submit
#[derive(Clone, Debug)]
pub enum SubmitDefinition {
    /// EVM contract submission
    NewEvmContract { chain_name: ChainName },

    /// Use an existing submit
    Submit(Submit),
}

/// Different types of input data
#[derive(Clone, Debug, Default)]
pub enum InputData {
    /// Raw bytes
    Raw(Vec<u8>),

    /// String data
    Text(String),

    /// Square request
    Square { x: u64 },

    /// Cosmos query
    CosmosQuery(CosmosQueryRequest),

    /// Permissions request
    Permissions(PermissionsRequest),

    /// No input data
    #[default]
    None,
}

impl InputData {
    /// Convert to bytes for sending to the trigger
    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        match self {
            InputData::Raw(data) => Some(data.clone()),
            InputData::Text(text) => Some(text.as_bytes().to_vec()),
            InputData::Square { x } => Some(serde_json::to_vec(&SquareRequest { x: *x }).unwrap()),
            InputData::CosmosQuery(req) => Some(req.to_vec()),
            InputData::Permissions(req) => Some(req.to_vec()),
            InputData::None => None,
        }
    }
}

/// Expected output from a test
#[derive(Clone, Debug)]
pub enum ExpectedOutput {
    /// Raw bytes
    Raw(Vec<u8>),

    /// String data
    Text(String),

    /// Square response
    Square { y: u64 },

    /// Same as the input data
    SameAsInput,

    /// Expect specific structure, but don't check values
    StructureOnly(OutputStructure),

    /// Accept any output
    Any,
}

/// For validating structure without checking values
#[derive(Clone, Debug)]
pub enum OutputStructure {
    CosmosQueryResponse,
    PermissionsResponse,
}

impl TestDefinition {
    /// Gets the service for this test, panicking if none is set
    pub fn get_service(&self) -> &Service {
        self.service
            .as_ref()
            .unwrap_or_else(|| panic!("Service not set for test: {}", self.name))
    }
}

/// Builder pattern for creating test definitions
pub struct TestBuilder {
    pub definition: TestDefinition,
}

impl TestBuilder {
    /// Create a new test builder with the given name
    pub fn new(name: &str) -> Self {
        Self {
            definition: TestDefinition {
                name: name.to_string(),
                description: None,
                workflows: BTreeMap::new(),
                service: None,
                service_manager_chain: ChainName::new("31337").unwrap(),
                priority: u64::MAX,
            },
        }
    }

    /// Add a description
    pub fn description(mut self, description: &str) -> Self {
        self.definition.description = Some(description.to_string());
        self
    }

    pub fn priority(mut self, priority: u64) -> Self {
        self.definition.priority = priority;
        self
    }

    /// Add a workflow
    pub fn add_workflow(
        mut self,
        workflow_id: WorkflowID,
        workflow: WorkflowConfig,
    ) -> anyhow::Result<Self> {
        if self.definition.workflows.contains_key(&workflow_id) {
            bail!("Workflow id {} is already in use", workflow_id)
        }
        self.definition.workflows.insert(workflow_id, workflow);
        Ok(self)
    }

    pub fn service_manager_chain(mut self, chain_name: &ChainName) -> Self {
        self.definition.service_manager_chain = chain_name.clone();
        self
    }

    /// Build the test definition
    pub fn build(self) -> TestDefinition {
        self.definition
    }
}

// Create a dedicated WorkflowBuilder to construct WorkflowConfig objects
#[derive(Default)]
pub struct WorkflowBuilder {
    components: Option<ComponentName>,
    trigger: Option<TriggerDefinition>,
    submit: Option<SubmitDefinition>,
    aggregators: Vec<AggregatorDefinition>,
    input_data: InputData,
    expected_output: Option<ExpectedOutput>,
}

impl WorkflowBuilder {
    /// Create a new workflow builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the components to use
    pub fn component(mut self, components: ComponentName) -> Self {
        self.components = Some(components);
        self
    }

    /// Configure an EVM contract trigger
    pub fn evm_trigger(mut self, evm_trigger: EvmTriggerDefinition) -> Self {
        self.trigger = Some(TriggerDefinition::Evm(evm_trigger));
        self
    }

    /// Use the previous workflow's trigger
    pub fn trigger(mut self, trigger: Trigger) -> Self {
        self.trigger = Some(TriggerDefinition::Trigger(trigger));
        self
    }

    /// Configure a Cosmos contract trigger
    pub fn cosmos_trigger(mut self, cosmos_trigger: CosmosTriggerDefinition) -> Self {
        self.trigger = Some(TriggerDefinition::Cosmos(cosmos_trigger));
        self
    }

    /// Configure a block interval trigger
    pub fn block_interval_trigger(
        mut self,
        chain_name: &ChainName,
        n_blocks: NonZeroU32,
        start_block: Option<NonZeroU64>,
        end_block: Option<NonZeroU64>,
    ) -> Self {
        self.trigger = Some(TriggerDefinition::Trigger(Trigger::BlockInterval {
            chain_name: chain_name.clone(),
            n_blocks,
            start_block,
            end_block,
        }));
        self
    }

    /// Configure a cron trigger
    pub fn cron_trigger(
        mut self,
        schedule: &str,
        start_time: Option<Timestamp>,
        end_time: Option<Timestamp>,
    ) -> Self {
        self.trigger = Some(TriggerDefinition::Trigger(Trigger::Cron {
            schedule: schedule.to_string(),
            start_time,
            end_time,
        }));
        self
    }

    /// Configure an EVM contract submit
    pub fn evm_submit(mut self, chain_name: &ChainName) -> Self {
        self.submit = Some(SubmitDefinition::NewEvmContract {
            chain_name: chain_name.clone(),
        });
        self
    }

    /// Configure an aggregator submit
    pub fn aggregator_submit(mut self, url: &str) -> Self {
        self.submit = Some(SubmitDefinition::Submit(Submit::Aggregator {
            url: url.to_string(),
        }));
        self
    }

    /// Configure with no submit
    pub fn no_submit(mut self) -> Self {
        self.submit = Some(SubmitDefinition::Submit(Submit::None));
        self
    }

    /// Add an aggregator
    pub fn add_aggregator(mut self, aggregator: AggregatorDefinition) -> Self {
        self.aggregators.push(aggregator);
        self
    }

    pub fn expect_output_structure(mut self, structure: OutputStructure) -> Self {
        self.expected_output = Some(ExpectedOutput::StructureOnly(structure));
        self
    }

    /// Build the workflow configuration
    pub fn build(self) -> anyhow::Result<WorkflowConfig> {
        let components = self.components.context("Components not set")?;
        let trigger = self.trigger.context("Trigger not set")?;
        let submit = self.submit.context("Submit not set")?;
        let expected_output = self.expected_output.context("Expected output not set")?;

        if let SubmitDefinition::Submit(Submit::Aggregator { .. }) = submit {
            ensure!(
                !self.aggregators.is_empty(),
                "No aggregators set when submit is aggregator"
            )
        }

        Ok(WorkflowConfig {
            component: components,
            trigger,
            submit,
            aggregators: self.aggregators,
            input_data: self.input_data,
            expected_output,
        })
    }

    /// Set raw input data
    pub fn input_data(mut self, data: Vec<u8>) -> Self {
        self.input_data = InputData::Raw(data);
        self
    }

    /// Set text input data
    pub fn input_text(mut self, text: &str) -> Self {
        self.input_data = InputData::Text(text.to_string());
        self
    }

    /// Set square input data
    pub fn input_square(mut self, x: u64) -> Self {
        self.input_data = InputData::Square { x };
        self
    }

    /// Set cosmos query input data
    pub fn input_cosmos_query(mut self, request: CosmosQueryRequest) -> Self {
        self.input_data = InputData::CosmosQuery(request);
        self
    }

    /// Set permissions input data
    pub fn input_permissions(mut self, request: PermissionsRequest) -> Self {
        self.input_data = InputData::Permissions(request);
        self
    }

    /// Set expected raw output
    pub fn expect_output(mut self, data: Vec<u8>) -> Self {
        self.expected_output = Some(ExpectedOutput::Raw(data));
        self
    }

    /// Set expected text output
    pub fn expect_text(mut self, text: &str) -> Self {
        self.expected_output = Some(ExpectedOutput::Text(text.to_string()));
        self
    }

    /// Set expected square output
    pub fn expect_square(mut self, y: u64) -> Self {
        self.expected_output = Some(ExpectedOutput::Square { y });
        self
    }

    /// Expect output to be the same as the input
    pub fn expect_same_output(mut self) -> Self {
        self.expected_output = Some(ExpectedOutput::SameAsInput);
        self
    }
}

/// Helper methods for testing the output
impl ExpectedOutput {
    /// Check if the actual output matches the expected output
    pub fn matches(&self, actual: &[u8], input: &InputData) -> bool {
        match self {
            ExpectedOutput::Raw(expected) => expected == actual,
            ExpectedOutput::Text(expected) => {
                if let Ok(actual_str) = std::str::from_utf8(actual) {
                    expected == actual_str
                } else {
                    false
                }
            }
            ExpectedOutput::Square { y } => {
                if let Ok(response) = serde_json::from_slice::<SquareResponse>(actual) {
                    &response.y == y
                } else {
                    false
                }
            }
            ExpectedOutput::SameAsInput => {
                if let Some(input_bytes) = input.to_bytes() {
                    input_bytes == actual
                } else {
                    false
                }
            }
            ExpectedOutput::StructureOnly(structure) => match structure {
                OutputStructure::CosmosQueryResponse => {
                    serde_json::from_slice::<CosmosQueryResponse>(actual).is_ok()
                }
                OutputStructure::PermissionsResponse => {
                    serde_json::from_slice::<PermissionsResponse>(actual).is_ok()
                }
            },
            ExpectedOutput::Any => true,
        }
    }
}
