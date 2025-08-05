use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
use wasm_pkg_client::{PackageRef, Version};
use wavs_types::{
    Aggregator, AllowedHostPermission, ChainName, ComponentDigest, CosmosContractSubmission,
    EvmContractSubmission, Permissions, ServiceStatus, Submit, Trigger, WorkflowID,
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

/// Result of setting a component's source to a digest
#[derive(Debug, Clone, Serialize)]
pub struct ComponentSourceDigestResult {
    /// The component digest
    pub digest: ComponentDigest,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentSourceDigestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component source set to digest successfully!")?;
        writeln!(f, "  Digest:       {}", self.digest)?;
        writeln!(f, "  Updated:      {}", self.file_path.display())
    }
}

/// Result of setting a component's source to a registry
#[derive(Debug, Clone, Serialize)]
pub struct ComponentSourceRegistryResult {
    /// The domain
    pub domain: String,
    /// The package reference
    pub package: PackageRef,
    /// The component digest (retrieved from registry)
    pub digest: ComponentDigest,
    /// The version
    pub version: Version,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentSourceRegistryResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component source set to registry package successfully!")?;
        writeln!(f, "  Domain:       {}", self.domain)?;
        writeln!(f, "  Package:      {}", self.package)?;
        writeln!(f, "  Version:      {}", self.version)?;
        writeln!(f, "  Digest:       {}", self.digest)?;
        writeln!(f, "  Updated:      {}", self.file_path.display())
    }
}

/// Result of updating a component's environment variables
#[derive(Debug, Clone, Serialize)]
pub struct ComponentEnvKeysResult {
    /// The updated environment variable keys
    pub env_keys: BTreeSet<String>,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentEnvKeysResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component environment variables updated successfully!")?;
        if self.env_keys.is_empty() {
            writeln!(f, "  Env Keys:    No environment variables")?;
        } else {
            writeln!(f, "  Env Keys:")?;
            for key in &self.env_keys {
                writeln!(f, "    {}", key)?;
            }
        }
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of adding a workflow
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowAddResult {
    /// The workflow id
    pub workflow_id: WorkflowID,
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
    pub workflow_id: WorkflowID,
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
    pub workflow_id: WorkflowID,
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
                chain_name,
                event_type,
            } => {
                writeln!(f, "  Trigger Type: Cosmos Contract Event")?;
                writeln!(f, "    Address:    {}", address)?;
                writeln!(f, "    Chain:      {}", chain_name)?;
                writeln!(f, "    Event Type: {}", event_type)?;
            }
            Trigger::EvmContractEvent {
                address,
                chain_name,
                event_hash,
            } => {
                writeln!(f, "  Trigger Type: EVM Contract Event")?;
                writeln!(f, "    Address:    {}", address)?;
                writeln!(f, "    Chain:      {}", chain_name)?;
                writeln!(f, "    Event Hash: {}", event_hash)?;
            }
            Trigger::Manual => {
                writeln!(f, "  Trigger Type: Manual")?;
            }
            Trigger::BlockInterval {
                chain_name,
                n_blocks,
                start_block,
                end_block,
            } => {
                writeln!(f, "  Trigger Type: Block Interval")?;
                writeln!(f, "    Chain:      {}", chain_name)?;
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
    pub workflow_id: WorkflowID,
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
                        chain_name,
                        address,
                        max_gas,
                    }) => writeln!(
                        f,
                        "    chain: {}, address: {}, max_gas: {}",
                        chain_name,
                        address,
                        max_gas
                            .map(|x| x.to_string())
                            .unwrap_or("default".to_string())
                    )?,
                    Aggregator::Cosmos(CosmosContractSubmission {
                        chain_name,
                        address,
                        max_gas,
                    }) => writeln!(
                        f,
                        "    chain: {}, address: {}, max_gas: {}",
                        chain_name,
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

/// Result of adding an aggregator handler
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowAddAggregatorResult {
    /// The workflow id that was updated
    pub workflow_id: WorkflowID,
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
                    chain_name,
                    address,
                    max_gas,
                }) => writeln!(
                    f,
                    "    chain: {}, address: {}, max_gas: {}",
                    chain_name,
                    address,
                    max_gas
                        .map(|x| x.to_string())
                        .unwrap_or("default".to_string())
                )?,
                Aggregator::Cosmos(CosmosContractSubmission {
                    chain_name,
                    address,
                    max_gas,
                }) => writeln!(
                    f,
                    "    chain: {}, address: {}, max_gas: {}",
                    chain_name,
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
    pub chain_name: ChainName,
    /// The EVM address
    pub address: alloy_primitives::Address,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for EvmManagerResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "EVM manager set successfully!")?;
        writeln!(f, "  Address:      {}", self.address)?;
        writeln!(f, "  Chain:        {}", self.chain_name)?;
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

/// Result of updating a component's fuel limit
#[derive(Debug, Clone, Serialize)]
pub struct ComponentFuelLimitResult {
    /// The updated fuel limit
    pub fuel_limit: Option<u64>,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentFuelLimitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component fuel limit updated successfully!")?;
        match self.fuel_limit {
            Some(limit) => writeln!(f, "  Fuel Limit:   {}", limit)?,
            None => writeln!(f, "  Fuel Limit:   No limit (removed)")?,
        }
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating a component's configuration
#[derive(Debug, Clone, Serialize)]
pub struct ComponentConfigResult {
    /// The updated configuration
    pub config: BTreeMap<String, String>,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentConfigResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component configuration updated successfully!")?;
        if self.config.is_empty() {
            writeln!(f, "  Config:      No configuration items")?;
        } else {
            writeln!(f, "  Config:")?;
            for (key, value) in &self.config {
                writeln!(f, "    {} => {}", key, value)?;
            }
        }
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating a component's maximum execution time
#[derive(Debug, Clone, Serialize)]
pub struct ComponentTimeLimitResult {
    /// The updated maximum execution time in seconds
    pub time_limit_seconds: Option<u64>,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentTimeLimitResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component maximum execution time updated successfully!")?;
        match self.time_limit_seconds {
            Some(seconds) => writeln!(f, "  Max Execution Time: {} seconds", seconds)?,
            None => writeln!(f, "  Max Execution Time: Default (no explicit limit)")?,
        }
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}

/// Result of updating component permissions
#[derive(Debug, Clone, Serialize)]
pub struct ComponentPermissionsResult {
    /// The updated permissions
    pub permissions: Permissions,
    /// The file path where the updated service JSON was saved
    pub file_path: PathBuf,
}

impl std::fmt::Display for ComponentPermissionsResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Component permissions updated successfully!")?;

        // Display HTTP permissions
        match &self.permissions.allowed_http_hosts {
            AllowedHostPermission::All => {
                writeln!(f, "  HTTP Hosts:   All allowed")?;
            }
            AllowedHostPermission::None => {
                writeln!(f, "  HTTP Hosts:   None allowed")?;
            }
            AllowedHostPermission::Only(hosts) => {
                writeln!(f, "  HTTP Hosts:   Only specific hosts allowed")?;
                for host in hosts {
                    writeln!(f, "    - {}", host)?;
                }
            }
        }

        // Display file system permission
        writeln!(
            f,
            "  File System: {}",
            if self.permissions.file_system {
                "Enabled"
            } else {
                "Disabled"
            }
        )?;
        writeln!(f, "  Updated:     {}", self.file_path.display())
    }
}
