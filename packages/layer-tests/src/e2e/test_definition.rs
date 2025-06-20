use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{anyhow, bail, ensure};
use regex::Regex;
use utils::config::WAVS_ENV_PREFIX;
use wavs_types::{ChainName, Trigger, WorkflowID};

use crate::e2e::components::ComponentName;
use crate::e2e::types::{
    CosmosQueryRequest, CosmosQueryResponse, PermissionsRequest, PermissionsResponse,
    SquareRequest, SquareResponse,
};

use super::config::DEFAULT_CHAIN_ID;

/// Defines a complete end-to-end test case
#[derive(Clone, Debug)]
pub struct TestDefinition {
    /// Unique name for this test
    pub name: String,

    /// Description of what this test verifies
    pub description: Option<String>,

    /// The workflows of this test
    pub workflows: BTreeMap<WorkflowID, WorkflowDefinition>,

    /// Service manager chain
    pub service_manager_chain: ChainName,

    /// Execution group (ascending priority)
    pub group: u64,
}

#[derive(Clone, Debug)]
pub struct ComponentDefinition {
    /// The name of the component
    pub name: ComponentName,

    /// Key-value pairs that are accessible in the components via host bindings.
    pub config_vars: BTreeMap<String, String>,

    /// External env variable keys to be read from the system host on execute (i.e. API keys).
    /// Must be prefixed with `WAVS_ENV_`.
    pub env_vars: BTreeMap<String, String>,
}

impl From<ComponentName> for ComponentDefinition {
    fn from(name: ComponentName) -> Self {
        ComponentDefinition {
            name,
            config_vars: BTreeMap::new(),
            env_vars: BTreeMap::new(),
        }
    }
}

impl ComponentName {
    pub fn into_builder(self) -> ComponentBuilder {
        ComponentBuilder::new(self)
    }
}

pub struct ComponentBuilder {
    name: ComponentName,
    config_vars: BTreeMap<String, String>,
    env_vars: BTreeMap<String, String>,
}

impl ComponentBuilder {
    pub fn new(name: ComponentName) -> Self {
        Self {
            name,
            config_vars: BTreeMap::new(),
            env_vars: BTreeMap::new(),
        }
    }

    pub fn with_config_var(mut self, key: String, value: String) -> Self {
        if self.env_vars.contains_key(&key) {
            panic!("Config var key '{}' is already defined", key);
        }

        self.config_vars.insert(key, value);
        self
    }

    pub fn with_env_var(mut self, key: String, value: String) -> Self {
        if !key.starts_with(WAVS_ENV_PREFIX) {
            panic!(
                "Env var key '{}' must be prefixed with '{WAVS_ENV_PREFIX}'",
                key
            );
        }
        if self.env_vars.contains_key(&key) {
            panic!("Env var key '{}' is already defined", key);
        }

        self.env_vars.insert(key, value);
        self
    }

    pub fn build(self) -> ComponentDefinition {
        ComponentDefinition {
            name: self.name,
            config_vars: self.config_vars,
            env_vars: self.env_vars,
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

    /// Aggregators configuration
    pub aggregators: Vec<AggregatorDefinition>,

    /// Input data to send to the trigger
    pub input_data: InputData,

    /// Expected output to verify
    pub expected_output: ExpectedOutput,

    /// The timeout for workflow to receive signed data
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub enum AggregatorDefinition {
    NewEvmAggregatorSubmit { chain_name: ChainName },
}

/// Configuration for a trigger
#[derive(Clone, Debug)]
pub enum TriggerDefinition {
    // Deploy a new EVM contract trigger for this test
    NewEvmContract(EvmTriggerDefinition),
    /// Deploy a new Cosmos contract trigger for this test
    NewCosmosContract(CosmosTriggerDefinition),
    /// Special case for block interval tests that need runtime block height calculation.
    /// Creates a block interval trigger with start_block=end_block=(current_height + delay)
    /// to test precise start/stop timing with n_blocks=1
    DeferredBlockIntervalTarget {
        chain_name: ChainName,
    },
    /// Use a pre-existing trigger without additional setup.
    /// Useful for multi-trigger tests where multiple workflows share the same trigger,
    /// or for standard triggers like cron/block intervals that don't need test-specific deployment
    Existing(Trigger),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CosmosTriggerDefinition {
    SimpleContractEvent { chain_name: ChainName },
}

#[derive(Clone, Debug)]
pub enum EvmTriggerDefinition {
    SimpleContractEvent { chain_name: ChainName },
}

/// Configuration for a submit
#[derive(Clone, Debug)]
pub enum SubmitDefinition {
    Aggregator { url: String },
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
    #[allow(dead_code)]
    Raw(Vec<u8>),
    /// String data
    Text(String),
    /// A regex match
    Regex(Regex),
    /// Square response
    Square { y: u64 },
    /// Expect specific structure, but don't check values
    StructureOnly(OutputStructure),
    /// Deferred value
    /// Block interval start stop uses this to dynamically expect a value
    Deferred,
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
                service_manager_chain: ChainName::new(DEFAULT_CHAIN_ID.to_string()).unwrap(),
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
    pub fn add_workflow(mut self, workflow_id: WorkflowID, workflow: WorkflowDefinition) -> Self {
        if self.definition.workflows.contains_key(&workflow_id) {
            panic!("Workflow id {} is already in use", workflow_id)
        }
        self.definition.workflows.insert(workflow_id, workflow);
        self
    }

    /// Set the service manager chain
    pub fn with_service_manager_chain(mut self, chain_name: &ChainName) -> Self {
        self.definition.service_manager_chain = chain_name.clone();
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
    aggregators: Vec<AggregatorDefinition>,
    input_data: InputData,
    expected_output: Option<ExpectedOutput>,
    timeout: Option<Duration>,
}

impl WorkflowBuilder {
    /// Create a new workflow builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the component to use
    pub fn with_component(mut self, component: ComponentDefinition) -> Self {
        if self.component.is_some() {
            panic!("Component already set");
        }
        self.component = Some(component);
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

    /// Add an aggregator
    pub fn with_aggregator(mut self, aggregator: AggregatorDefinition) -> Self {
        self.aggregators.push(aggregator);
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

    /// Build the workflow definition
    pub fn build(self) -> WorkflowDefinition {
        let component = self.component.expect("Component not set");
        let trigger = self.trigger.expect("Trigger not set");
        let submit = self.submit.expect("Submit not set");
        let expected_output = self.expected_output.expect("Expected output not set");

        let SubmitDefinition::Aggregator { .. } = submit;
        if self.aggregators.is_empty() {
            panic!("No aggregators set when submit is aggregator")
        }

        WorkflowDefinition {
            component,
            trigger,
            submit,
            aggregators: self.aggregators,
            input_data: self.input_data,
            expected_output,
            timeout: self.timeout.unwrap_or(Duration::from_secs(15)),
        }
    }
}

/// Helper methods for testing the output
impl ExpectedOutput {
    /// Check if the actual output matches the expected output
    pub fn validate(&self, actual: &[u8]) -> anyhow::Result<()> {
        let is_valid = match self {
            ExpectedOutput::Raw(expected) => expected == actual,
            ExpectedOutput::Text(expected) => {
                let actual_str = std::str::from_utf8(actual)?;
                expected == actual_str
            }
            ExpectedOutput::Regex(regex) => {
                let actual_str = std::str::from_utf8(actual)?;
                regex.is_match(actual_str)
            }
            ExpectedOutput::Square { y } => {
                if let Ok(response) = serde_json::from_slice::<SquareResponse>(actual) {
                    &response.y == y
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
            ExpectedOutput::Deferred => {
                bail!("Invalid configuration: Deferred values must be set dynamically")
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
