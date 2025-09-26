use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use wasm_pkg_client::{PackageRef, Version};
use wavs_types::{
    Aggregator, ChainKey, ComponentDigest, EvmContractSubmission, Permissions, ServiceStatus,
    Submit, Trigger, WorkflowId,
};

use crate::service_json::ServiceJson;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChainType {
    Cosmos,
    EVM,
}

/// Result of service initialization
#[derive(Debug, Clone, Serialize)]
pub struct ServiceInitResult {
    /// The generated service
    pub service: ServiceJson,
    /// The file path where the service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ServiceInitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Service JSON generated successfully!")?;
        writeln!(f, "  Name: {}", self.service.name)?;
        writeln!(f, "  File: {}", self.file_path.display())
    }
}

/// Result of adding a workflow
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowAddResult {
    /// The workflow id
    pub workflow_id: WorkflowId,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowAddResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow added successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of deleting a workflow
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowDeleteResult {
    /// The workflow id that was deleted
    pub workflow_id: WorkflowId,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowDeleteResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow deleted successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating a workflow's trigger
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowTriggerResult {
    /// The workflow id that was updated
    pub workflow_id: WorkflowId,
    /// The updated trigger type
    pub trigger: Trigger,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowTriggerResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow trigger updated successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;

        match &self.trigger {
            Trigger::CosmosContractEvent {
                address,
                chain,
                event_type,
            } => {
                writeln!(f, "  Trigger Type: Cosmos Contract Event")?;
                writeln!(f, "    Address:    {}", address)?;
                writeln!(f, "    Chain:      {}", chain)?;
                writeln!(f, "    Event Type: {}", event_type)?;
            }
            Trigger::EvmContractEvent {
                address,
                chain,
                event_hash,
            } => {
                writeln!(f, "  Trigger Type: EVM Contract Event")?;
                writeln!(f, "    Address:    {}", address)?;
                writeln!(f, "    Chain:      {}", chain)?;
                writeln!(f, "    Event Hash: {}", event_hash)?;
            }
            Trigger::Manual => {
                writeln!(f, "  Trigger Type: Manual")?;
            }
            Trigger::BlockInterval {
                chain,
                n_blocks,
                start_block,
                end_block,
            } => {
                writeln!(f, "  Trigger Type: Block Interval")?;
                writeln!(f, "    Chain:      {}", chain)?;
                writeln!(f, "    Interval:   {} blocks", n_blocks)?;
                if let Some(start) = start_block {
                    writeln!(f, "    Start Block: {}", u64::from(*start))?;
                } else {
                    writeln!(f, "    Start Block: None")?;
                }
                if let Some(end) = end_block {
                    writeln!(f, "    End Block:   {}", u64::from(*end))?;
                } else {
                    writeln!(f, "    End Block:   None")?;
                }
            }
            Trigger::Cron {
                schedule,
                start_time,
                end_time,
            } => {
                writeln!(f, "  Trigger Type: Cron")?;
                writeln!(f, "    Schedule:   {}", schedule)?;
                if let Some(start) = start_time {
                    writeln!(f, "    Start Time: {}", start.as_nanos())?;
                } else {
                    writeln!(f, "    Start Time: None")?;
                }
                if let Some(end) = end_time {
                    writeln!(f, "    End Time:   {}", end.as_nanos())?;
                } else {
                    writeln!(f, "    End Time:   None")?;
                }
            }
        }

        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating a workflow's submit
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowSetSubmitAggregatorResult {
    /// The workflow id that was updated
    pub workflow_id: WorkflowId,
    /// The updated submit type
    pub submit: Submit,
    /// The aggregator submit
    pub aggregator_submit: Aggregator,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowSetSubmitAggregatorResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow submit updated successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;

        match &self.submit {
            Submit::None => {
                writeln!(f, "  Submit Type: None")?;
            }
            Submit::Aggregator { url, .. } => {
                writeln!(f, "  Submit Type: Aggregator")?;
                writeln!(f, "    Url:    {}", url)?;
                match &self.aggregator_submit {
                    Aggregator::Evm(EvmContractSubmission {
                        chain,
                        address,
                        max_gas,
                    }) => writeln!(
                        f,
                        "    chain: {}, address: {}, max_gas: {}",
                        chain,
                        address,
                        max_gas
                            .map(|x| x.to_string())
                            .unwrap_or("default".to_string())
                    )?,
                }
            }
        }

        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of setting the submit to None
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowSetSubmitNoneResult {
    /// The workflow id that was updated
    pub workflow_id: WorkflowId,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

/// Result of setting an aggregator URL
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowSetAggregatorUrlResult {
    /// The workflow id that was updated
    pub workflow_id: WorkflowId,
    /// The aggregator URL that was set
    pub url: String,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowSetSubmitNoneResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow submit set to None successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

impl std::fmt::Display for WorkflowSetAggregatorUrlResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow aggregator URL set successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;
        writeln!(f, "  URL:         {}", self.url)?;
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of adding an aggregator handler
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowAddAggregatorResult {
    /// The workflow id that was updated
    pub workflow_id: WorkflowId,
    /// The updated submit type
    pub aggregator_submits: Vec<Aggregator>,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for WorkflowAddAggregatorResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Workflow aggregator submit updated successfully!")?;
        writeln!(f, "  Workflow ID: {}", self.workflow_id)?;

        writeln!(f, "  Aggregators: ")?;
        for submit in &self.aggregator_submits {
            match submit {
                Aggregator::Evm(EvmContractSubmission {
                    chain,
                    address,
                    max_gas,
                }) => writeln!(
                    f,
                    "    chain: {}, address: {}, max_gas: {}",
                    chain,
                    address,
                    max_gas
                        .map(|x| x.to_string())
                        .unwrap_or("default".to_string())
                )?,
            }
        }

        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of setting the EVM manager
#[derive(Debug, Clone, Serialize)]
pub struct EvmManagerResult {
    /// The EVM chain name
    pub chain: ChainKey,
    /// The EVM address
    pub address: alloy_primitives::Address,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for EvmManagerResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "EVM manager set successfully!")?;
        writeln!(f, "  Address:      {}", self.address)?;
        writeln!(f, "  Chain:        {}", self.chain)?;
        writeln!(f, "  Updated:      {}", self.file_path.display())
    }
}

/// Result of updating the service status
#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatusResult {
    /// The updated status
    pub status: ServiceStatus,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for UpdateStatusResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Status updated successfully!")?;
        writeln!(f, "  Status:        {:#?}", self.status)?;
        writeln!(f, "  Updated:      {}", self.file_path.display())
    }
}

/// Result of service validation
#[derive(Debug, Clone, Serialize)]
pub struct ServiceValidationResult {
    /// The service name
    pub service_name: String,
    /// Any errors generated during validation
    pub errors: Vec<String>,
}

impl std::fmt::Display for ServiceValidationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.errors.is_empty() {
            writeln!(f, "✅ Service validation successful!")?;
            writeln!(f, "   Service Name: {}", self.service_name)?;
        } else {
            writeln!(f, "❌ Service validation failed with errors")?;
            writeln!(f, "   Service Name: {}", self.service_name)?;
            writeln!(f, "   Errors:")?;
            for (i, error) in self.errors.iter().enumerate() {
                writeln!(f, "   {}: {}", i + 1, error)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum ComponentContext {
    Workflow { workflow_id: WorkflowId },
    Aggregator { workflow_id: WorkflowId },
}

impl std::fmt::Display for ComponentContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentContext::Workflow { workflow_id } => {
                write!(f, "Workflow Component (ID: {})", workflow_id)
            }
            ComponentContext::Aggregator { workflow_id } => {
                write!(f, "Aggregator Component (Workflow ID: {})", workflow_id)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum ComponentOperationResult {
    SourceUrl {
        context: ComponentContext,
        digest: ComponentDigest,
        file_path: PathBuf,
        url: String,
    },
    SourceDigest {
        context: ComponentContext,
        digest: ComponentDigest,
        file_path: PathBuf,
    },
    SourceRegistry {
        context: ComponentContext,
        domain: String,
        package: PackageRef,
        digest: ComponentDigest,
        version: Version,
        file_path: PathBuf,
    },
    Permissions {
        context: ComponentContext,
        permissions: Permissions,
        file_path: PathBuf,
    },
    FuelLimit {
        context: ComponentContext,
        fuel_limit: Option<u64>,
        file_path: PathBuf,
    },
    Config {
        context: ComponentContext,
        config: BTreeMap<String, String>,
        file_path: PathBuf,
    },
    TimeLimit {
        context: ComponentContext,
        time_limit_seconds: Option<u64>,
        file_path: PathBuf,
    },
    EnvKeys {
        context: ComponentContext,
        env_keys: BTreeSet<String>,
        file_path: PathBuf,
    },
}

impl ComponentOperationResult {
    /// Get the file path from any variant
    pub fn file_path(&self) -> &std::path::PathBuf {
        match self {
            ComponentOperationResult::SourceDigest { file_path, .. } => file_path,
            ComponentOperationResult::SourceRegistry { file_path, .. } => file_path,
            ComponentOperationResult::SourceUrl { file_path, .. } => file_path,
            ComponentOperationResult::Permissions { file_path, .. } => file_path,
            ComponentOperationResult::FuelLimit { file_path, .. } => file_path,
            ComponentOperationResult::Config { file_path, .. } => file_path,
            ComponentOperationResult::TimeLimit { file_path, .. } => file_path,
            ComponentOperationResult::EnvKeys { file_path, .. } => file_path,
        }
    }

    /// Get the workflow ID from any variant (extracts from context)
    pub fn workflow_id(&self) -> &wavs_types::WorkflowId {
        match self {
            ComponentOperationResult::SourceUrl { context, .. } => match context {
                ComponentContext::Workflow { workflow_id } => workflow_id,
                ComponentContext::Aggregator { workflow_id } => workflow_id,
            },
            ComponentOperationResult::SourceDigest { context, .. } => match context {
                ComponentContext::Workflow { workflow_id } => workflow_id,
                ComponentContext::Aggregator { workflow_id } => workflow_id,
            },
            ComponentOperationResult::SourceRegistry { context, .. } => match context {
                ComponentContext::Workflow { workflow_id } => workflow_id,
                ComponentContext::Aggregator { workflow_id } => workflow_id,
            },
            ComponentOperationResult::Permissions { context, .. } => match context {
                ComponentContext::Workflow { workflow_id } => workflow_id,
                ComponentContext::Aggregator { workflow_id } => workflow_id,
            },
            ComponentOperationResult::FuelLimit { context, .. } => match context {
                ComponentContext::Workflow { workflow_id } => workflow_id,
                ComponentContext::Aggregator { workflow_id } => workflow_id,
            },
            ComponentOperationResult::Config { context, .. } => match context {
                ComponentContext::Workflow { workflow_id } => workflow_id,
                ComponentContext::Aggregator { workflow_id } => workflow_id,
            },
            ComponentOperationResult::TimeLimit { context, .. } => match context {
                ComponentContext::Workflow { workflow_id } => workflow_id,
                ComponentContext::Aggregator { workflow_id } => workflow_id,
            },
            ComponentOperationResult::EnvKeys { context, .. } => match context {
                ComponentContext::Workflow { workflow_id } => workflow_id,
                ComponentContext::Aggregator { workflow_id } => workflow_id,
            },
        }
    }
}

impl std::fmt::Display for ComponentOperationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentOperationResult::SourceUrl {
                context,
                digest,
                file_path,
                url,
            } => {
                writeln!(f, "{} source set to url successfully!", context)?;
                writeln!(f, "  Url:          {}", url)?;
                writeln!(f, "  Digest:       {}", digest)?;
                writeln!(f, "  Updated:      {}", file_path.display())
            }
            ComponentOperationResult::SourceDigest {
                context,
                digest,
                file_path,
            } => {
                writeln!(f, "{} source set to digest successfully!", context)?;
                writeln!(f, "  Digest:       {}", digest)?;
                writeln!(f, "  Updated:      {}", file_path.display())
            }

            ComponentOperationResult::SourceRegistry {
                context,
                domain,
                package,
                version,
                digest,
                file_path,
            } => {
                writeln!(
                    f,
                    "{} source set to registry package successfully!",
                    context
                )?;
                writeln!(f, "  Domain:       {}", domain)?;
                writeln!(f, "  Package:      {}", package)?;
                writeln!(f, "  Version:      {}", version)?;
                writeln!(f, "  Digest:       {}", digest)?;
                writeln!(f, "  Updated:      {}", file_path.display())
            }

            ComponentOperationResult::Permissions {
                context,
                permissions,
                file_path,
            } => {
                writeln!(f, "{} permissions updated successfully!", context)?;
                writeln!(f, "  HTTP Hosts:   {:?}", permissions.allowed_http_hosts)?;
                writeln!(f, "  File System:  {}", permissions.file_system)?;
                writeln!(f, "  Updated:      {}", file_path.display())
            }

            ComponentOperationResult::FuelLimit {
                context,
                fuel_limit,
                file_path,
            } => {
                writeln!(f, "{} fuel limit updated successfully!", context)?;
                match fuel_limit {
                    Some(limit) => writeln!(f, "  Fuel Limit:   {}", limit)?,
                    None => writeln!(f, "  Fuel Limit:   No limit (removed)")?,
                }
                writeln!(f, "  Updated:      {}", file_path.display())
            }

            ComponentOperationResult::Config {
                context,
                config,
                file_path,
            } => {
                writeln!(f, "{} configuration updated successfully!", context)?;
                if config.is_empty() {
                    writeln!(f, "  Config:       No configuration items")?;
                } else {
                    writeln!(f, "  Config:")?;
                    for (key, value) in config {
                        writeln!(f, "    {} => {}", key, value)?;
                    }
                }
                writeln!(f, "  Updated:      {}", file_path.display())
            }

            ComponentOperationResult::TimeLimit {
                context,
                time_limit_seconds,
                file_path,
            } => {
                writeln!(f, "{} time limit updated successfully!", context)?;
                match time_limit_seconds {
                    Some(seconds) => writeln!(f, "  Max Time:     {} seconds", seconds)?,
                    None => writeln!(f, "  Max Time:     Default (no explicit limit)")?,
                }
                writeln!(f, "  Updated:      {}", file_path.display())
            }

            ComponentOperationResult::EnvKeys {
                context,
                env_keys,
                file_path,
            } => {
                writeln!(f, "{} environment variables updated successfully!", context)?;
                if env_keys.is_empty() {
                    writeln!(f, "  Env Keys:     No environment variables")?;
                } else {
                    writeln!(f, "  Env Keys:")?;
                    for key in env_keys {
                        writeln!(f, "    {}", key)?;
                    }
                }
                writeln!(f, "  Updated:      {}", file_path.display())
            }
        }
    }
}
