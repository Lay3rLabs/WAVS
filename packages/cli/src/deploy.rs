use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Display};
use wavs_types::{ChainName, Service, ServiceID, Submit, Trigger, WorkflowID};

use crate::config::Config;

// Commands that return a type which update the deployment should implement this
pub trait CommandDeployResult: Display {
    fn update_deployment(&self, deployment: &mut Deployment);
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Deployment {
    pub eth_service_managers: BTreeMap<ChainName, Vec<alloy_primitives::Address>>,
    pub services: BTreeMap<ServiceID, Service>,
}

impl Deployment {
    pub fn load(config: &Config, json: bool) -> Result<Self> {
        let path = Self::path(config);

        if !json {
            tracing::debug!("Loading deployment from {}", path.display());
        }

        if !path.exists() {
            if !json {
                tracing::warn!("No deployment file found at {:?}, using default", path);
            }
            return Ok(Self::default());
        }

        let file = std::fs::File::open(&path).map_err(|e| {
            anyhow!(
                "unable to open CLI deployments file at {}: {}",
                path.display(),
                e
            )
        })?;
        let reader = std::io::BufReader::new(file);
        let deployment = serde_json::from_reader(reader).map_err(|e| {
            anyhow!(
                "unable to parse CLI deployments file at {}: {}",
                path.display(),
                e
            )
        })?;

        Ok(deployment)
    }

    pub fn path(config: &Config) -> std::path::PathBuf {
        config.data.join("deployments.json")
    }

    pub fn get_trigger(
        &self,
        service_id: &ServiceID,
        workflow_id: Option<&WorkflowID>,
    ) -> Option<Trigger> {
        let service = self.services.get(service_id)?;
        let workflow = match workflow_id {
            Some(workflow_id) => service.workflows.get(workflow_id)?,
            None => service.workflows.values().next()?,
        };

        Some(workflow.trigger.clone())
    }

    pub fn get_submit(
        &self,
        service_id: &ServiceID,
        workflow_id: Option<&WorkflowID>,
    ) -> Option<Submit> {
        let service = self.services.get(service_id)?;
        let workflow = match workflow_id {
            Some(workflow_id) => service.workflows.get(workflow_id)?,
            None => service.workflows.values().next()?,
        };

        Some(workflow.submit.clone())
    }
}
