use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

use wavs_types::{ByteArray, ChainName, ServiceID, TriggerConfig, WorkflowID};

use super::schedulers::{block_scheduler::BlockSchedulers, cron_scheduler::CronScheduler};

#[allow(clippy::type_complexity)]
pub struct LookupMaps {
    /// single lookup for all triggers (in theory, can be more than just task queue addr)
    pub trigger_configs: Arc<RwLock<BTreeMap<LookupId, TriggerConfig>>>,
    /// lookup id by (chain name, contract event address, event type)
    pub triggers_by_cosmos_contract_event:
        Arc<RwLock<HashMap<(ChainName, layer_climb::prelude::Address, String), HashSet<LookupId>>>>,
    /// lookup id by (chain id, contract event address, event hash)
    pub triggers_by_evm_contract_event: Arc<
        RwLock<HashMap<(ChainName, alloy_primitives::Address, ByteArray<32>), HashSet<LookupId>>>,
    >,
    /// Efficient block schedulers (one per chain) for block interval triggers
    pub block_schedulers: BlockSchedulers,
    /// lookup id by service id -> workflow id
    pub triggers_by_service_workflow:
        Arc<RwLock<BTreeMap<ServiceID, BTreeMap<WorkflowID, LookupId>>>>,
    /// latest lookup_id
    pub lookup_id: Arc<AtomicUsize>,
    /// cron scheduler
    pub cron_scheduler: CronScheduler,
}

impl Default for LookupMaps {
    fn default() -> Self {
        Self::new()
    }
}

impl LookupMaps {
    pub fn new() -> Self {
        Self {
            trigger_configs: Arc::new(RwLock::new(BTreeMap::new())),
            lookup_id: Arc::new(AtomicUsize::new(0)),
            triggers_by_cosmos_contract_event: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_evm_contract_event: Arc::new(RwLock::new(HashMap::new())),
            block_schedulers: BlockSchedulers::default(),
            triggers_by_service_workflow: Arc::new(RwLock::new(BTreeMap::new())),
            cron_scheduler: CronScheduler::default(),
        }
    }
}

pub type LookupId = usize;
