use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wavs_types::{Service, Workflow, WorkflowId};

#[derive(Clone, Debug)]
pub struct ServiceDeployment {
    pub service: Service,
    pub submission_handlers: BTreeMap<WorkflowId, layer_climb::prelude::Address>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WorkflowDeployment {
    pub workflow: Workflow,
    pub submission_handler: layer_climb::prelude::Address,
}
