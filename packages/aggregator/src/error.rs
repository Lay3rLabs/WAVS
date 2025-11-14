use thiserror::Error;
use utils::{
    error::EvmClientError,
    storage::{db::DBError, CAStorageError},
};
use wavs_engine::utils::error::EngineError;
use wavs_types::{
    contracts::cosmwasm::service_manager::error::WavsValidateError, ChainConfigError, ChainKey,
    ChainKeyError, EnvelopeError, ServiceId, ServiceManagerError, WorkflowId, WorkflowIdError,
};

pub type AggregatorResult<T> = Result<T, AggregatorError>;

#[derive(Error, Debug)]
pub enum AggregatorError {
    #[error("Missing workflow: {workflow_id} for service: {service_id}")]
    MissingWorkflow {
        workflow_id: WorkflowId,
        service_id: ServiceId,
    },

    #[error("DB: {0}")]
    DBError(#[from] DBError),

    #[error("Packet Validation: {0}")]
    PacketValidation(#[from] PacketValidationError),

    #[error("Chain Config: {0}")]
    ChainConfig(#[from] ChainConfigError),

    #[error("Join: {0}")]
    JoinError(String),

    #[error("Evm: {0}")]
    EvmClient(#[from] EvmClientError),

    #[error("Envelope: {0}")]
    Envelope(#[from] EnvelopeError),

    #[error("Evm client create: {0:?}")]
    CreateEvmClient(anyhow::Error),

    #[error("Service manager validate(): {0:?}")]
    CosmosServiceManagerValidate(WavsValidateError),

    #[error("Service manager validate(): {0:?}")]
    EvmServiceManagerValidateKnown(ServiceManagerError),

    #[error("Service manager validate(): {0}")]
    EvmServiceManagerValidateAnyRevert(String),

    #[error("Service manager validate(): {0:?}")]
    EvmServiceManagerValidateUnknown(alloy_contract::Error),

    #[error("Chain not found: {0}")]
    ChainNotFound(ChainKey),

    #[error("Missing EVM credential")]
    MissingEvmCredential,

    #[error("Missing Cosmos credential")]
    MissingCosmosCredential,

    #[error("Unexpected responses length: should be {responses}, got {aggregators}")]
    UnexpectedResponsesLength {
        responses: usize,
        aggregators: usize,
    },

    #[error("Block number: {0}")]
    BlockNumber(anyhow::Error),

    #[error("Failed to encode with bincode: {0:?}")]
    BincodeEncode(#[from] bincode::error::EncodeError),

    #[error("Failed to decode with bincode: {0:?}")]
    BincodeDecode(#[from] bincode::error::DecodeError),

    #[error("Unable to fetch service: {0:?}")]
    FetchService(anyhow::Error),

    #[error("Evm: Unable to look up operator key from signing key: {0:?}")]
    EvmOperatorKeyLookup(alloy_contract::Error),

    #[error("Cosmos: Unable to look up operator key from signing key")]
    CosmosOperatorKeyLookup,

    #[error("Unable to look up service manager from service handler: {0:?}")]
    EvmServiceManagerLookup(alloy_contract::Error),

    #[error("Service already registered: {0}")]
    RepeatService(ServiceId),

    #[error("No such service registered: {0}")]
    MissingService(ServiceId),

    #[error("WASM component compilation failed: {0}")]
    WasmCompilation(#[source] wasmtime::Error),

    #[error("Component execution failed: {0}")]
    ComponentExecution(String),

    #[error("Component loading failed: {0}")]
    ComponentLoad(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Engine initialization failed: {0}")]
    EngineInitialization(String),

    #[error("WASM engine error: {0}")]
    WasmEngine(#[from] EngineError),

    #[error("Storage error: {0}")]
    Storage(#[from] CAStorageError),

    #[error("Invalid Workflow ID: {0}")]
    InvalidWorkflowId(#[from] WorkflowIdError),

    #[error("Invalid ChainKey: {0}")]
    InvalidChainKey(#[from] ChainKeyError),
}

#[derive(Error, Debug)]
pub enum PacketValidationError {
    #[error("Unexpected envelope difference")]
    EnvelopeDiff,

    #[error("Could not parse submit action: {0}")]
    ParseSubmitAction(String),
}
