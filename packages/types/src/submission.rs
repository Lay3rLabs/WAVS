use serde::{Deserialize, Serialize};

use crate::{Envelope, EventId, ServiceId, TriggerAction, WasmResponse, WavsSignature, WorkflowId};

// The data we send from submission to dispatcher
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Submission {
    pub trigger_action: TriggerAction,
    pub operator_response: WasmResponse,
    pub event_id: EventId,
    pub envelope: Envelope,
    pub envelope_signature: WavsSignature,
}

impl Submission {
    pub fn label(&self) -> String {
        format!(
            "Submission(service_id: {}, workflow_id: {}, event_id: {})",
            self.service_id(),
            self.workflow_id(),
            self.event_id
        )
    }
    pub fn service_id(&self) -> &ServiceId {
        &self.trigger_action.config.service_id
    }

    pub fn workflow_id(&self) -> &WorkflowId {
        &self.trigger_action.config.workflow_id
    }
}
