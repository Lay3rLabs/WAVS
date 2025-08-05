use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wavs_types::{Service, Workflow, WorkflowID};

#[derive(Clone, Debug)]
pub struct ServiceDeployment {
    pub service: Service,
    pub submission_handlers: BTreeMap<WorkflowID, alloy_primitives::Address>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WorkflowDeployment {
    pub workflow: Workflow,
    pub submission_handler: alloy_primitives::Address,
}
