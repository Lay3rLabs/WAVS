use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, ensure};
use example_types::{
    CosmosQueryRequest, CosmosQueryResponse, KvStoreRequest, KvStoreResponse, PermissionsRequest,
    PermissionsResponse, SquareRequest, SquareResponse,
};
use regex::Regex;
use wavs_types::{ChainKey, Trigger, WorkflowId};

use crate::e2e::components::{
    AggregatorComponent, ComponentName, ComponentSources, OperatorComponent,
};

/// Defines a complete end-to-end test case
#[derive(Clone, Debug)]
pub struct TestDefinition {
    /// Unique name for this test
    pub name: String,

    /// Description of what this test verifies
    pub description: Option<String>,

    /// The workflows of this test
    pub workflows: BTreeMap<WorkflowId, WorkflowDefinition>,

    /// If a service changes, set it here
    /// the change will be applied after service deployment
    /// but before explicit trigger and test evaluation
    pub change_service: Option<ChangeServiceDefinition>,

    /// Service manager chain
    pub service_manager_chain: Option<ChainKey>,

    /// Execution group (ascending priority)
    pub group: u64,
}

#[derive(Clone, Debug)]
pub struct ComponentDefinition {
    /// The name of the component
    pub name: ComponentName,

    pub configs_to_add: ComponentConfigsToAdd,
    pub env_vars_to_add: HashMap<String, String>,
}

impl ComponentDefinition {
    pub fn with_config_hardcoded(mut self, key: String, value: String) -> Self {
        self.configs_to_add.hardcoded.insert(key, value);
        self
    }

    pub fn with_config_service_handler(mut self) -> Self {
        self.configs_to_add.service_handler = true;
        self
    }

    pub fn with_env_var(mut self, key: String, value: String) -> Self {
        self.env_vars_to_add.insert(key, value);
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct ComponentConfigsToAdd {
    pub service_handler: bool,
    pub hardcoded: HashMap<String, String>,
}

impl From<ComponentName> for ComponentDefinition {
    fn from(name: ComponentName) -> Self {
        ComponentDefinition {
            name,
            configs_to_add: ComponentConfigsToAdd::default(),
            env_vars_to_add: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct WorkflowDefinition {
    /// Component configuration
    pub component: ComponentDefinition,

    /// Trigger configuration
    pub trigger: TriggerDefinition,

    /// Submit configuration
    pub submit: SubmitDefinition,

    /// Input data to send to the trigger
    pub input_data: InputData,

    /// Expected output to verify
    pub expected_output: ExpectedOutput,

    /// The timeout for workflow to receive signed data
    pub timeout: Duration,

    /// Aggregator components needed for this workflow
    pub aggregators: Vec<ComponentName>,

    /// Configuration for how trigger execution should behave during tests
    pub trigger_execution: TriggerExecutionConfig,
}

impl WorkflowDefinition {
    pub fn expects_reorg(&self) -> bool {
        matches!(self.expected_output, ExpectedOutput::Dropped)
    }
}

#[derive(Clone, Debug, Default)]
pub struct TriggerExecutionConfig {
    pub log_spam_count: usize,
}

#[derive(Clone, Debug)]
pub enum AggregatorDefinition {
    ComponentBasedAggregator {
        component: ComponentDefinition,
        chain: ChainKey,
    },
}

#[derive(Clone, Debug)]
pub enum ChangeServiceDefinition {
    Component {
        workflow_id: WorkflowId,
        component: ComponentDefinition,
    },
    AddWorkflow {
        workflow_id: WorkflowId,
        workflow: WorkflowDefinition,
    },
    // TODO: status etc.
}

/// Configuration for a trigger
#[derive(Clone, Debug)]
pub enum TriggerDefinition {
    // Deploy a new EVM contract trigger for this test
    NewEvmContract(EvmTriggerDefinition),
    /// Deploy a new Cosmos contract trigger for this test
    NewCosmosContract(CosmosTriggerDefinition),
    /// Special case for block interval tests that need runtime block height calculation.
    BlockInterval {
        chain: ChainKey,
        /// Set the start and stop to the same block height, effectively creating a one-shot trigger.
        start_stop: bool,
    },
    /// Use a pre-existing trigger without additional setup.
    /// Useful for multi-trigger tests where multiple workflows share the same trigger,
    Existing(Trigger),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CosmosTriggerDefinition {
    SimpleContractEvent { chain: ChainKey },
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CosmosSubmitDefinition {
    MockServiceHandler { chain: ChainKey },
}

#[derive(Clone, Debug)]
pub enum EvmTriggerDefinition {
    SimpleContractEvent { chain: ChainKey },
}

/// Configuration for a submit
#[derive(Clone, Debug)]
pub enum SubmitDefinition {
    Aggregator {
        url: String,
        aggregator: AggregatorDefinition,
    },
}

/// Different types of input data
#[derive(Clone, Debug, Default)]
pub enum InputData {
    /// Raw bytes
    #[allow(dead_code)]
    Raw(Vec<u8>),
    /// String data
    Text(String),
    /// Square request
    Square(SquareRequest),
    /// KvStore request
    KvStore(KvStoreRequest),
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
            InputData::Square(req) => Some(req.to_vec()),
            InputData::KvStore(req) => Some(req.to_vec()),
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
    #[allow(dead_code)]
    Raw(Vec<u8>),
    /// String data
    Text(String),
    /// A regex match
    #[allow(dead_code)]
    Regex(Regex),
    /// Square response
    Square(SquareResponse),
    KvStore(KvStoreResponse),
    /// Square input, echoed back (used in "change service" tests)
    EchoSquare {
        x: u64,
    },
    /// Expect specific structure, but don't check values
    StructureOnly(OutputStructure),
    /// For a dynamic callback that checks the output
    Callback(Arc<dyn ExpectedOutputCallback>),
    /// Expect no output (transaction dropped due to re-org)
    Dropped,
}

pub trait ExpectedOutputCallback: Send + Sync + std::fmt::Debug + 'static {
    /// Validate the actual output against the expected output
    fn validate(
        &self,
        test: &TestDefinition,
        clients: &super::clients::Clients,
        component_sources: &ComponentSources,
        actual: &[u8],
    ) -> anyhow::Result<()>;
}

/// For validating structure without checking values
#[derive(Clone, Debug)]
pub enum OutputStructure {
    CosmosQueryResponse,
    PermissionsResponse,
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
                service_manager_chain: None,
                change_service: None,
                group: u64::MAX,
            },
        }
    }

    /// Add a description
    pub fn with_description(mut self, description: &str) -> Self {
        self.definition.description = Some(description.to_string());
        self
    }

    /// Set the execution group
    pub fn with_group(mut self, group: u64) -> Self {
        self.definition.group = group;
        self
    }

    /// Add a workflow
    pub fn add_workflow(mut self, workflow_id: WorkflowId, workflow: WorkflowDefinition) -> Self {
        if self.definition.workflows.contains_key(&workflow_id) {
            panic!("Workflow id {} is already in use", workflow_id)
        }
        self.definition.workflows.insert(workflow_id, workflow);
        self
    }

    pub fn with_change_service(mut self, change: ChangeServiceDefinition) -> Self {
        if self.definition.change_service.is_some() {
            panic!("Change service already set");
        }
        self.definition.change_service = Some(change);
        self
    }

    /// Set the service manager chain
    pub fn with_service_manager_chain(mut self, chain: &ChainKey) -> Self {
        self.definition.service_manager_chain = Some(chain.clone());
        self
    }

    /// Build the test definition
    pub fn build(self) -> TestDefinition {
        self.definition
    }
}

/// Simplified workflow builder with overwrite protection
#[derive(Default)]
pub struct WorkflowBuilder {
    component: Option<ComponentDefinition>,
    trigger: Option<TriggerDefinition>,
    submit: Option<SubmitDefinition>,
    input_data: InputData,
    expected_output: Option<ExpectedOutput>,
    timeout: Option<Duration>,
    aggregators: Vec<ComponentName>,
    trigger_execution: TriggerExecutionConfig,
}

impl WorkflowBuilder {
    /// Create a new workflow builder
    pub fn new() -> Self {
        Self {
            trigger_execution: TriggerExecutionConfig::default(),
            ..Default::default()
        }
    }

    /// Set the operator component to use
    pub fn with_operator_component(mut self, component: OperatorComponent) -> Self {
        if self.component.is_some() {
            panic!("Component already set");
        }
        self.component = Some(ComponentDefinition::from(ComponentName::Operator(
            component,
        )));
        self
    }

    /// Set the trigger definition
    pub fn with_trigger(mut self, trigger: TriggerDefinition) -> Self {
        if self.trigger.is_some() {
            panic!("Trigger already set");
        }
        self.trigger = Some(trigger);
        self
    }

    /// Set the submit definition
    pub fn with_submit(mut self, submit: SubmitDefinition) -> Self {
        if self.submit.is_some() {
            panic!("Submit already set");
        }
        self.submit = Some(submit);
        self
    }

    /// Set the input data
    pub fn with_input_data(mut self, input_data: InputData) -> Self {
        self.input_data = input_data;
        self
    }

    /// Set the expected output
    pub fn with_expected_output(mut self, expected_output: ExpectedOutput) -> Self {
        if self.expected_output.is_some() {
            panic!("Expected output already set");
        }
        self.expected_output = Some(expected_output);
        self
    }

    /// Set the timeout
    #[allow(dead_code)]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        if self.timeout.is_some() {
            panic!("Timeout already set");
        }
        self.timeout = Some(timeout);
        self
    }

    /// Add an aggregator component for this workflow
    pub fn with_aggregator_component(mut self, aggregator: AggregatorComponent) -> Self {
        self.aggregators.push(ComponentName::Aggregator(aggregator));
        self
    }

    /// Emit additional unrelated EVM logs to stress the trigger stream
    pub fn with_log_spam_count(mut self, count: usize) -> Self {
        self.trigger_execution.log_spam_count = count;
        self
    }

    /// Build the workflow definition
    pub fn build(self) -> WorkflowDefinition {
        let component = self.component.expect("Component not set");
        let trigger = self.trigger.expect("Trigger not set");
        let submit = self.submit.expect("Submit not set");
        let expected_output = self.expected_output.expect("Expected output not set");

        let SubmitDefinition::Aggregator { .. } = &submit;

        WorkflowDefinition {
            component,
            trigger,
            submit,
            input_data: self.input_data,
            expected_output,
            timeout: self.timeout.unwrap_or(Duration::from_secs(120)),
            aggregators: self.aggregators,
            trigger_execution: self.trigger_execution,
        }
    }
}

/// Helper methods for testing the output
impl ExpectedOutput {
    /// Check if the actual output matches the expected output
    pub fn validate(
        &self,
        test: &TestDefinition,
        clients: &super::clients::Clients,
        component_sources: &ComponentSources,
        actual: &[u8],
    ) -> anyhow::Result<()> {
        let is_valid = match self {
            ExpectedOutput::Raw(expected) => expected == actual,
            ExpectedOutput::Text(expected) => {
                let actual_str = std::str::from_utf8(actual)?;
                tracing::info!("Text response: {actual_str}");
                expected == actual_str
            }
            ExpectedOutput::Regex(regex) => {
                let actual_str = std::str::from_utf8(actual)?;
                regex.is_match(actual_str)
            }
            ExpectedOutput::Square(expected) => {
                if let Ok(response) = serde_json::from_slice::<SquareResponse>(actual) {
                    tracing::info!("Square response: {response:?}");
                    response.y == expected.y
                } else {
                    false
                }
            }
            ExpectedOutput::KvStore(expected) => {
                if let Ok(response) = serde_json::from_slice::<KvStoreResponse>(actual) {
                    tracing::info!("KvStore response: {response:?}");
                    response == *expected
                } else {
                    false
                }
            }
            ExpectedOutput::EchoSquare { x } => {
                if let Ok(response) = serde_json::from_slice::<SquareRequest>(actual) {
                    tracing::info!("Echo square response: {response:?}");
                    &response.x == x
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
            ExpectedOutput::Callback(callback) => {
                callback.validate(test, clients, component_sources, actual)?;
                return Ok(());
            }
            ExpectedOutput::Dropped => {
                // For dropped transactions, validate we got no output
                actual.is_empty()
            }
        };

        ensure!(
            is_valid,
            anyhow!(
                "Expected {:?}, Received {}",
                self,
                std::str::from_utf8(actual)?
            )
        );

        Ok(())
    }
}
