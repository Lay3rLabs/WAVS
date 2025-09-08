use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wavs_types::{Service, Workflow, WorkflowId};

#[derive(Clone, Debug)]
pub struct ServiceDeployment {
    pub service: Service,
    pub submission_handlers: BTreeMap<WorkflowId, alloy_primitives::Address>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WorkflowDeployment {
    pub workflow: Workflow,
    pub submission_handler: alloy_primitives::Address,
}
