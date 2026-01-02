use wavs_types::{
    Envelope, EventId, EventIdSalt, Service, ServiceId, TriggerAction, WasmResponse, WavsSignature,
    WorkflowId,
};

/// The data we send from engine to submission, after operator component execution
#[derive(Clone, Debug)]
pub struct SubmissionRequest {
    pub trigger_action: TriggerAction,
    pub operator_response: WasmResponse,
    pub service: Service,
    #[cfg(feature = "dev")]
    pub debug: SubmissionRequestDebug,
}

impl SubmissionRequest {
    pub fn service_id(&self) -> &ServiceId {
        &self.trigger_action.config.service_id
    }

    pub fn workflow_id(&self) -> &WorkflowId {
        &self.trigger_action.config.workflow_id
    }

    pub fn event_id(&self) -> Result<EventId, bincode::error::EncodeError> {
        match &self.operator_response.event_id_salt {
            Some(salt) => EventId::new(
                self.service_id(),
                self.workflow_id(),
                EventIdSalt::WasmResponse(&salt),
            ),
            None => EventId::new(
                self.service_id(),
                self.workflow_id(),
                EventIdSalt::Trigger(&self.trigger_action.data),
            ),
        }
    }
}

#[cfg(feature = "dev")]
#[derive(Clone, Debug, Default)]
// these debug-only fields are used to control behavior during testing
pub struct SubmissionRequestDebug {
    // Do not submit to aggregator, even if it's defined
    pub do_not_submit_aggregator: bool,
}

// The data we send from submission to dispatcher
#[derive(Clone, Debug)]
pub struct Submission {
    pub trigger_action: TriggerAction,
    pub operator_response: WasmResponse,
    pub event_id: EventId,
    pub envelope: Envelope,
    pub envelope_signature: WavsSignature,
}

impl Submission {
    pub fn service_id(&self) -> &ServiceId {
        &self.trigger_action.config.service_id
    }

    pub fn workflow_id(&self) -> &WorkflowId {
        &self.trigger_action.config.workflow_id
    }
}
