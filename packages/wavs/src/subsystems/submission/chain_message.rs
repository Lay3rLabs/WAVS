use wavs_types::{Envelope, ServiceId, Submit, WorkflowId};

/// The data returned from a trigger action
#[derive(Clone, Debug)]
pub struct ChainMessage {
    pub service_id: ServiceId,
    pub workflow_id: WorkflowId,
    pub envelope: Envelope,
    pub submit: Submit,
    pub origin_tx_hash: Vec<u8>,
    pub origin_block: u64,
    #[cfg(debug_assertions)]
    pub debug: ChainMessageDebug,
}

#[cfg(debug_assertions)]
#[derive(Clone, Debug, Default)]
// these debug-only fields are used to control behavior during testing
pub struct ChainMessageDebug {
    // Do not submit to aggregator, even if it's defined
    pub do_not_submit_aggregator: bool,
}
