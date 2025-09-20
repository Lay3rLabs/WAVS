use wavs_types::{Envelope, ServiceId, Submit, TriggerData, WorkflowId};

/// The data returned from a trigger action
#[derive(Clone, Debug)]
pub struct ChainMessage {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub envelope: Envelope,
    pub trigger_data: TriggerData,
    pub submit: Submit,
    #[cfg(feature = "dev")]
    pub debug: ChainMessageDebug,
}

#[cfg(feature = "dev")]
#[derive(Clone, Debug, Default)]
// these debug-only fields are used to control behavior during testing
pub struct ChainMessageDebug {
    // Do not submit to aggregator, even if it's defined
    pub do_not_submit_aggregator: bool,
}
