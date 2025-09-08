use wavs_types::{Envelope, ServiceId, Submit, WorkflowId};

/// The data returned from a trigger action
#[derive(Clone, Debug)]
pub struct ChainMessage {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub envelope: Envelope,
    pub submit: Submit,
}
