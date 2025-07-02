use wavs_types::{Envelope, ServiceID, Submit, WorkflowID};

/// The data returned from a trigger action
#[derive(Clone, Debug)]
pub struct ChainMessage {
    pub service_id: ServiceID,
    pub workflow_id: WorkflowID,
    pub envelope: Envelope,
    pub submit: Submit,
}
