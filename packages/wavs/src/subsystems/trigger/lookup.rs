use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

use bimap::BiMap;
use utils::telemetry::TriggerMetrics;
use wavs_types::{
    AtProtoAction, ByteArray, ChainKey, ServiceId, Trigger, TriggerConfig, WorkflowId,
};

use crate::{
    services::Services,
    subsystems::trigger::{
        error::TriggerError,
        schedulers::{block_scheduler::BlockIntervalState, cron_scheduler::CronIntervalState},
    },
};

use super::schedulers::{block_scheduler::BlockSchedulers, cron_scheduler::CronScheduler};

#[allow(clippy::type_complexity)]
pub struct LookupMaps {
    /// single lookup for all triggers (in theory, can be more than just task queue addr)
    trigger_configs: Arc<RwLock<BTreeMap<LookupId, TriggerConfig>>>,
    services: Services,
    metrics: TriggerMetrics,
    /// lookup id by (chain name, contract event address, event type)
    pub triggers_by_cosmos_contract_event: Arc<
        RwLock<HashMap<(ChainKey, layer_climb::prelude::CosmosAddr, String), HashSet<LookupId>>>,
    >,
    /// lookup id by (chain id, contract event address, event hash)
    pub triggers_by_evm_contract_event: Arc<
        RwLock<HashMap<(ChainKey, alloy_primitives::Address, ByteArray<32>), HashSet<LookupId>>>,
    >,
    /// lookup id by (collection, optional repo_did, optional action) for exact matches
    pub triggers_by_atproto_event_exact:
        Arc<RwLock<HashMap<(String, Option<String>, Option<AtProtoAction>), HashSet<LookupId>>>>,
    /// lookup id by (collection pattern, optional repo_did, optional action) for wildcard matches
    pub triggers_by_atproto_event_pattern:
        Arc<RwLock<HashMap<(String, Option<String>, Option<AtProtoAction>), HashSet<LookupId>>>>,
    // ServiceId <-> ServiceManager address
    pub service_manager: Arc<RwLock<BiMap<ServiceId, layer_climb::prelude::Address>>>,
    /// Efficient block schedulers (one per chain) for block interval triggers
    pub block_schedulers: BlockSchedulers,
    /// lookup id by service id -> workflow id
    pub triggers_by_service_workflow:
        Arc<RwLock<BTreeMap<ServiceId, BTreeMap<WorkflowId, LookupId>>>>,
    /// latest lookup_id
    pub lookup_id: Arc<AtomicUsize>,
    /// cron scheduler
    pub cron_scheduler: CronScheduler,
}

impl LookupMaps {
    pub fn new(services: Services, metrics: TriggerMetrics) -> Self {
        Self {
            trigger_configs: Arc::new(RwLock::new(BTreeMap::new())),
            lookup_id: Arc::new(AtomicUsize::new(0)),
            triggers_by_cosmos_contract_event: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_evm_contract_event: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_atproto_event_exact: Arc::new(RwLock::new(HashMap::new())),
            triggers_by_atproto_event_pattern: Arc::new(RwLock::new(HashMap::new())),
            block_schedulers: BlockSchedulers::default(),
            triggers_by_service_workflow: Arc::new(RwLock::new(BTreeMap::new())),
            service_manager: Arc::new(RwLock::new(BiMap::new())),
            cron_scheduler: CronScheduler::default(),
            services,
            metrics,
        }
    }

    pub fn get_trigger_config(&self, lookup_id: LookupId) -> Option<TriggerConfig> {
        let trigger_configs = self.trigger_configs.read().unwrap();
        let trigger_config = match trigger_configs.get(&lookup_id).cloned() {
            Some(config) => config,
            None => {
                self.metrics
                    .increment_total_errors("trigger config not found");
                tracing::error!("Trigger config not found for lookup_id {}", lookup_id);
                return None;
            }
        };

        match self.services.is_active(&trigger_config.service_id) {
            true => Some(trigger_config),
            _ => None,
        }
    }

    pub fn get_trigger_configs<'a>(
        &self,
        lookup_ids: impl IntoIterator<Item = &'a LookupId>,
    ) -> Vec<TriggerConfig> {
        let trigger_configs = self.trigger_configs.read().unwrap();
        lookup_ids
            .into_iter()
            .filter_map(|id| match trigger_configs.get(id) {
                Some(config) => Some(config.clone()),
                None => {
                    self.metrics
                        .increment_total_errors("trigger config not found");
                    tracing::error!("Trigger config not found for lookup_id {}", id);
                    None
                }
            })
            .filter(|config| self.services.is_active(&config.service_id))
            .collect()
    }

    pub fn add_service(&self, service: &wavs_types::Service) -> Result<(), TriggerError> {
        let manager_address: layer_climb::prelude::Address = service.manager.address();

        self.service_manager
            .write()
            .unwrap()
            .insert(service.id(), manager_address);

        for (id, workflow) in &service.workflows {
            let trigger = TriggerConfig {
                service_id: service.id(),
                workflow_id: id.clone(),
                trigger: workflow.trigger.clone(),
            };
            self.add_trigger(trigger)?;
        }

        Ok(())
    }

    pub fn add_trigger(&self, config: TriggerConfig) -> Result<(), TriggerError> {
        // get the next lookup id
        let lookup_id = self
            .lookup_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        match config.trigger.clone() {
            Trigger::EvmContractEvent {
                address,
                chain,
                event_hash,
            } => {
                let key = (chain.clone(), address, event_hash);
                self.triggers_by_evm_contract_event
                    .write()
                    .unwrap()
                    .entry(key)
                    .or_default()
                    .insert(lookup_id);
            }
            Trigger::CosmosContractEvent {
                address,
                chain,
                event_type,
            } => {
                let key = (chain.clone(), address.clone(), event_type.clone());
                self.triggers_by_cosmos_contract_event
                    .write()
                    .unwrap()
                    .entry(key)
                    .or_default()
                    .insert(lookup_id);
            }
            Trigger::BlockInterval {
                chain,
                n_blocks,
                start_block,
                end_block,
            } => {
                self.block_schedulers
                    .entry(chain.clone())
                    .or_default()
                    .add_trigger(BlockIntervalState::new(
                        lookup_id,
                        n_blocks,
                        start_block.map(Into::into),
                        end_block.map(Into::into),
                    ))?;
            }
            Trigger::Cron {
                schedule,
                start_time,
                end_time,
            } => {
                // Add directly to the cron scheduler
                self.cron_scheduler
                    .lock()
                    .unwrap()
                    .add_trigger(CronIntervalState::new(
                        lookup_id, &schedule, start_time, end_time,
                    )?)?;
            }
            Trigger::AtProtoEvent {
                collection,
                repo_did,
                action,
            } => {
                let key = (collection.clone(), repo_did.clone(), action.clone());
                // Use separate collections so the pattern-matching path only iterates over patterns
                if collection.contains('*') {
                    self.triggers_by_atproto_event_pattern
                        .write()
                        .unwrap()
                        .entry(key)
                        .or_default()
                        .insert(lookup_id);
                } else {
                    self.triggers_by_atproto_event_exact
                        .write()
                        .unwrap()
                        .entry(key)
                        .or_default()
                        .insert(lookup_id);
                }
            }
            Trigger::Manual => {}
        }

        // adding it to our lookups is the same, regardless of type
        self.triggers_by_service_workflow
            .write()
            .unwrap()
            .entry(config.service_id.clone())
            .or_default()
            .insert(config.workflow_id.clone(), lookup_id);

        self.trigger_configs
            .write()
            .unwrap()
            .insert(lookup_id, config);

        Ok(())
    }

    pub fn remove_workflow(
        &self,
        service_id: ServiceId,
        workflow_id: WorkflowId,
    ) -> Result<(), TriggerError> {
        let mut service_lock = self.triggers_by_service_workflow.write().unwrap();

        let workflow_map = service_lock
            .get_mut(&service_id)
            .ok_or_else(|| TriggerError::NoSuchService(service_id.clone()))?;

        // first remove it from services
        let lookup_id = workflow_map
            .remove(&workflow_id)
            .ok_or(TriggerError::NoSuchWorkflow(service_id, workflow_id))?;

        // Get the trigger type to know which scheduler to remove from
        let trigger_type = {
            let trigger_configs = self.trigger_configs.read().unwrap();
            trigger_configs
                .get(&lookup_id)
                .map(|config| config.trigger.clone())
        };

        // Remove from the appropriate collection based on trigger type
        if let Some(trigger) = trigger_type {
            match trigger {
                Trigger::EvmContractEvent {
                    address,
                    chain,
                    event_hash,
                } => {
                    let mut lock = self.triggers_by_evm_contract_event.write().unwrap();
                    if let Some(set) = lock.get_mut(&(chain.clone(), address, event_hash)) {
                        set.remove(&lookup_id);
                        if set.is_empty() {
                            lock.remove(&(chain, address, event_hash));
                        }
                    }
                }
                Trigger::CosmosContractEvent {
                    address,
                    chain,
                    event_type,
                } => {
                    let mut lock = self.triggers_by_cosmos_contract_event.write().unwrap();
                    if let Some(set) =
                        lock.get_mut(&(chain.clone(), address.clone(), event_type.clone()))
                    {
                        set.remove(&lookup_id);
                        if set.is_empty() {
                            lock.remove(&(chain, address, event_type));
                        }
                    }
                }
                Trigger::BlockInterval { chain, .. } => {
                    // Remove from block scheduler
                    if let Some(mut scheduler) = self.block_schedulers.get_mut(&chain) {
                        scheduler.remove_trigger(lookup_id);
                    }
                }
                Trigger::Cron { .. } => {
                    // Remove from cron scheduler
                    self.cron_scheduler
                        .lock()
                        .unwrap()
                        .remove_trigger(lookup_id);
                }
                Trigger::Manual => {}
                Trigger::AtProtoEvent {
                    collection,
                    repo_did,
                    action,
                } => {
                    let key = (collection.clone(), repo_did.clone(), action.clone());
                    if collection.contains('*') {
                        let mut lock = self.triggers_by_atproto_event_pattern.write().unwrap();
                        if let Some(set) = lock.get_mut(&key) {
                            set.remove(&lookup_id);
                            if set.is_empty() {
                                lock.remove(&key);
                            }
                        }
                    } else {
                        let mut lock = self.triggers_by_atproto_event_exact.write().unwrap();
                        if let Some(set) = lock.get_mut(&key) {
                            set.remove(&lookup_id);
                            if set.is_empty() {
                                lock.remove(&key);
                            }
                        }
                    }
                }
            }
        }

        // Remove from trigger_configs
        self.trigger_configs.write().unwrap().remove(&lookup_id);

        Ok(())
    }

    pub fn remove_service(&self, service_id: wavs_types::ServiceId) -> Result<(), TriggerError> {
        let mut trigger_configs = self.trigger_configs.write().unwrap();
        let mut triggers_by_evm_contract_event =
            self.triggers_by_evm_contract_event.write().unwrap();
        let mut triggers_by_cosmos_contract_event =
            self.triggers_by_cosmos_contract_event.write().unwrap();
        let mut triggers_by_atproto_event_exact =
            self.triggers_by_atproto_event_exact.write().unwrap();
        let mut triggers_by_atproto_event_pattern =
            self.triggers_by_atproto_event_pattern.write().unwrap();
        let mut triggers_by_service_workflow_lock =
            self.triggers_by_service_workflow.write().unwrap();

        // Remove the service manager
        self.service_manager
            .write()
            .unwrap()
            .remove_by_left(&service_id);

        if let Some(workflow_map) = triggers_by_service_workflow_lock.get(&service_id) {
            // Collect all lookup IDs to be removed
            let lookup_ids: Vec<LookupId> = workflow_map.values().copied().collect();

            // Remove triggers from all collections
            for lookup_id in &lookup_ids {
                if let Some(config) = trigger_configs.get(lookup_id) {
                    match &config.trigger {
                        Trigger::EvmContractEvent {
                            address,
                            chain,
                            event_hash,
                        } => {
                            if let Some(set) = triggers_by_evm_contract_event.get_mut(&(
                                chain.clone(),
                                *address,
                                *event_hash,
                            )) {
                                set.remove(lookup_id);
                                if set.is_empty() {
                                    triggers_by_evm_contract_event.remove(&(
                                        chain.clone(),
                                        *address,
                                        *event_hash,
                                    ));
                                }
                            }
                        }
                        Trigger::CosmosContractEvent {
                            address,
                            chain,
                            event_type,
                        } => {
                            if let Some(set) = triggers_by_cosmos_contract_event.get_mut(&(
                                chain.clone(),
                                address.clone(),
                                event_type.clone(),
                            )) {
                                set.remove(lookup_id);
                                if set.is_empty() {
                                    triggers_by_cosmos_contract_event.remove(&(
                                        chain.clone(),
                                        address.clone(),
                                        event_type.clone(),
                                    ));
                                }
                            }
                        }
                        Trigger::BlockInterval { chain, .. } => {
                            // Remove from block scheduler
                            if let Some(mut scheduler) = self.block_schedulers.get_mut(chain) {
                                scheduler.remove_trigger(*lookup_id);
                            }
                        }
                        Trigger::Cron { .. } => {
                            self.cron_scheduler
                                .lock()
                                .unwrap()
                                .remove_trigger(*lookup_id);
                        }
                        Trigger::Manual => {}
                        Trigger::AtProtoEvent {
                            collection,
                            repo_did,
                            action,
                        } => {
                            let key = (collection.clone(), repo_did.clone(), action.clone());
                            if collection.contains('*') {
                                if let Some(set) = triggers_by_atproto_event_pattern.get_mut(&key) {
                                    set.remove(lookup_id);
                                    if set.is_empty() {
                                        triggers_by_atproto_event_pattern.remove(&key);
                                    }
                                }
                            } else if let Some(set) = triggers_by_atproto_event_exact.get_mut(&key)
                            {
                                set.remove(lookup_id);
                                if set.is_empty() {
                                    triggers_by_atproto_event_exact.remove(&key);
                                }
                            }
                        }
                    }
                }
            }

            // Remove all trigger configs
            for lookup_id in &lookup_ids {
                trigger_configs.remove(lookup_id);
            }

            // Remove from service_workflow_lookup_map
            triggers_by_service_workflow_lock.remove(&service_id);
        }

        Ok(())
    }

    pub fn configs_for_service(
        &self,
        service_id: ServiceId,
    ) -> Result<Vec<TriggerConfig>, TriggerError> {
        let mut triggers = Vec::new();

        let triggers_by_service_workflow_lock = self.triggers_by_service_workflow.read().unwrap();
        let trigger_configs = self.trigger_configs.read().unwrap();

        let workflow_map = triggers_by_service_workflow_lock
            .get(&service_id)
            .ok_or(TriggerError::NoSuchService(service_id))?;

        for lookup_id in workflow_map.values() {
            let trigger_config = trigger_configs
                .get(lookup_id)
                .ok_or(TriggerError::NoSuchTriggerData(*lookup_id))?;
            triggers.push(trigger_config.clone());
        }

        Ok(triggers)
    }
}

pub type LookupId = usize;
