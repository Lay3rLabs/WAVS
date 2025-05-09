use std::time::Duration;

use wavs_types::{ChainName, Submit, Trigger, TriggerConfig};

use crate::e2e::components::ComponentName;
use crate::e2e::runner::{CosmosQueryRequest, PermissionsRequest, SquareRequest, SquareResponse};

/// Defines a complete end-to-end test case
#[derive(Clone, Debug)]
pub struct TestDefinition {
    /// Unique name for this test
    pub name: String,

    /// Description of what this test verifies
    pub description: Option<String>,

    /// Components used in this test
    pub components: Vec<ComponentName>,

    /// Trigger configuration
    pub trigger: TriggerConfig,

    /// Submit configuration
    pub submit: SubmitConfig,

    /// Input data to send to the trigger
    pub input_data: InputData,

    /// Expected output to verify
    pub expected_output: ExpectedOutput,

    /// Timeout for this test
    pub timeout: Duration,

    /// Whether to test with multiple triggers
    pub use_multi_trigger: bool,

    /// Number of tasks to execute (for aggregator tests)
    pub num_tasks: u32,
}

/// Configuration for a submit
#[derive(Clone, Debug)]
pub enum SubmitConfig {
    /// EVM contract submission
    EvmContract { chain_name: ChainName },

    /// Aggregator submission
    Aggregator { chain_name: ChainName },

    /// No submission
    None,

    /// Use an existing submit
    UseExisting { submit: Submit },
}

/// Different types of input data
#[derive(Clone, Debug)]
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
                components: Vec::new(),
                trigger: TriggerConfig::EvmContract {
                    chain_name: ChainName::new("31337").unwrap(),
                },
                submit: SubmitConfig::EvmContract {
                    chain_name: ChainName::new("31337").unwrap(),
                },
                input_data: InputData::None,
                expected_output: ExpectedOutput::Any,
                timeout: Duration::from_secs(5),
                use_multi_trigger: false,
                num_tasks: 1,
            },
        }
    }

    /// Add a description
    pub fn description(mut self, description: &str) -> Self {
        self.definition.description = Some(description.to_string());
        self
    }

    /// Add a component
    pub fn component(mut self, component: ComponentName) -> Self {
        self.definition.components.push(component);
        self
    }

    /// Set components
    pub fn components(mut self, components: Vec<ComponentName>) -> Self {
        self.definition.components = components;
        self
    }

    /// Configure an EVM contract trigger
    pub fn evm_trigger(mut self, chain_name: &str) -> Self {
        self.definition.trigger = TriggerConfig::EvmContract {
            chain_name: ChainName::new(chain_name).unwrap(),
        };
        self
    }

    /// Configure a Cosmos contract trigger
    pub fn cosmos_trigger(mut self, chain_name: &str) -> Self {
        self.definition.trigger = TriggerConfig::CosmosContract {
            chain_name: ChainName::new(chain_name).unwrap(),
        };
        self
    }

    /// Configure a block interval trigger
    pub fn block_interval_trigger(mut self, chain_name: &str, n_blocks: u32) -> Self {
        self.definition.trigger = TriggerConfig::BlockInterval {
            chain_name: ChainName::new(chain_name).unwrap(),
            n_blocks,
        };
        self
    }

    /// Configure a cron trigger
    pub fn cron_trigger(mut self, schedule: &str) -> Self {
        self.definition.trigger = TriggerConfig::Cron {
            schedule: schedule.to_string(),
        };
        self
    }

    /// Configure an EVM contract submit
    pub fn evm_submit(mut self, chain_name: &str) -> Self {
        self.definition.submit = SubmitConfig::EvmContract {
            chain_name: ChainName::new(chain_name).unwrap(),
        };
        self
    }

    /// Configure an aggregator submit
    pub fn aggregator_submit(mut self, chain_name: &str) -> Self {
        self.definition.submit = SubmitConfig::Aggregator {
            chain_name: ChainName::new(chain_name).unwrap(),
        };
        self
    }

    /// Configure with no submit
    pub fn no_submit(mut self) -> Self {
        self.definition.submit = SubmitConfig::None;
        self
    }

    /// Set raw input data
    pub fn input_data(mut self, data: Vec<u8>) -> Self {
        self.definition.input_data = InputData::Raw(data);
        self
    }

    /// Set text input data
    pub fn input_text(mut self, text: &str) -> Self {
        self.definition.input_data = InputData::Text(text.to_string());
        self
    }

    /// Set square input data
    pub fn input_square(mut self, x: u64) -> Self {
        self.definition.input_data = InputData::Square { x };
        self
    }

    /// Set cosmos query input data
    pub fn input_cosmos_query(mut self, request: CosmosQueryRequest) -> Self {
        self.definition.input_data = InputData::CosmosQuery(request);
        self
    }

    /// Set permissions input data
    pub fn input_permissions(mut self, request: PermissionsRequest) -> Self {
        self.definition.input_data = InputData::Permissions(request);
        self
    }

    /// Set expected raw output
    pub fn expect_output(mut self, data: Vec<u8>) -> Self {
        self.definition.expected_output = ExpectedOutput::Raw(data);
        self
    }

    /// Set expected text output
    pub fn expect_text(mut self, text: &str) -> Self {
        self.definition.expected_output = ExpectedOutput::Text(text.to_string());
        self
    }

    /// Set expected square output
    pub fn expect_square(mut self, y: u64) -> Self {
        self.definition.expected_output = ExpectedOutput::Square { y };
        self
    }

    /// Expect output to be the same as the input
    pub fn expect_same_output(mut self) -> Self {
        self.definition.expected_output = ExpectedOutput::SameAsInput;
        self
    }

    /// Set test timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.definition.timeout = timeout;
        self
    }

    /// Enable multi-trigger testing
    pub fn with_multi_trigger(mut self) -> Self {
        self.definition.use_multi_trigger = true;
        self
    }

    /// Set the number of tasks to execute (for aggregator tests)
    pub fn num_tasks(mut self, num_tasks: u32) -> Self {
        self.definition.num_tasks = num_tasks;
        self
    }

    /// Build the test definition
    pub fn build(self) -> TestDefinition {
        self.definition
    }
}

impl Default for TestDefinition {
    fn default() -> Self {
        TestBuilder::new("default_test").build()
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
                    &input_bytes == actual
                } else {
                    false
                }
            }
            ExpectedOutput::StructureOnly(structure) => match structure {
                OutputStructure::CosmosQueryResponse => {
                    serde_json::from_slice::<crate::e2e::runner::CosmosQueryResponse>(actual)
                        .is_ok()
                }
                OutputStructure::PermissionsResponse => {
                    serde_json::from_slice::<crate::e2e::runner::PermissionsResponse>(actual)
                        .is_ok()
                }
            },
            ExpectedOutput::Any => true,
        }
    }
}
