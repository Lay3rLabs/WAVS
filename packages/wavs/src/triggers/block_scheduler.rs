use dashmap::DashMap;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::num::NonZero;
use wavs_types::ChainName;

use super::core::LookupId;

/// Configuration for a block-based trigger.
#[derive(Copy, Clone, Debug)]
pub struct BlockTriggerConfig {
    /// Number of blocks between each trigger firing. Must be > 0.
    pub n_blocks: NonZero<u32>,
    /// Optional block height at which the trigger should begin.
    pub start_block: Option<NonZero<u64>>,
    /// Optional block height after which the trigger becomes inactive.
    pub end_block: Option<NonZero<u64>>,
}

/// Internal state of a scheduled block trigger.
#[derive(Copy, Clone, Debug)]
struct TriggerState {
    /// Next block height at which this trigger should fire.
    /// None means the trigger is not yet initialized
    next_block: Option<u64>,
    /// Trigger configuration.
    config: BlockTriggerConfig,
}

/// The `BlockScheduler` manages recurring triggers that are activated every `n_blocks`.
/// Triggers are optional bounded by a start and end block height.
/// Internally, it's organized as:
/// - A mapping from block height → trigger IDs (who fires when)
/// - A mapping from trigger ID → its schedule and state
/// - A sorted set of scheduled block heights
/// - A set of uninitialized triggers waiting for their start block
#[derive(Debug, Default)]
pub struct BlockScheduler {
    /// Maps block heights to sets of trigger IDs
    triggers_by_block: HashMap<u64, HashSet<LookupId>>,
    /// Keeps track of block heights in order
    scheduled_blocks: BTreeSet<u64>,
    /// Mapping of lookup ID to its current status
    trigger_status: HashMap<LookupId, TriggerState>,
    /// Uninitialized triggers waiting for a block to reference
    uninitialized_triggers: HashSet<LookupId>,
}

impl BlockScheduler {
    /// Schedules a new trigger without requiring current block height.
    /// If start_block is specified, first firing will be at that exact block.
    ///
    /// # Arguments
    /// * `lookup_id` - Unique identifier for this trigger
    /// * `config` - Configuration for the trigger
    ///
    /// # Returns
    /// * `true` if the trigger was successfully scheduled
    /// * `false` if a trigger with this ID already exists
    pub fn schedule_trigger(&mut self, lookup_id: LookupId, config: BlockTriggerConfig) -> bool {
        // Don't allow duplicate lookup IDs
        if self.trigger_status.contains_key(&lookup_id) {
            return false;
        }

        // If start_block is specified, we can initialize immediately
        if let Some(start) = config.start_block {
            let start_block = start.get();

            // Store trigger status with the exact start block
            self.trigger_status.insert(
                lookup_id,
                TriggerState {
                    next_block: Some(start_block),
                    config,
                },
            );

            // Add to the map and set
            self.triggers_by_block
                .entry(start_block)
                .or_default()
                .insert(lookup_id);
            self.scheduled_blocks.insert(start_block);
        } else {
            // No start_block specified, add to uninitialized set
            self.trigger_status.insert(
                lookup_id,
                TriggerState {
                    next_block: None, // Explicitly uninitialized
                    config,
                },
            );

            self.uninitialized_triggers.insert(lookup_id);
        }

        true
    }

    /// Process a block and return all triggers that should fire at this block height.
    ///
    /// # Arguments
    /// * `block_height` - The current block height
    ///
    /// # Returns
    /// Vector of lookup IDs that should fire at this block height
    pub fn process_block(&mut self, block_height: u64) -> Vec<LookupId> {
        let mut to_fire = Vec::new();

        // Initialize uninitialized triggers
        let uninitialized_ids: Vec<_> = self.uninitialized_triggers.iter().cloned().collect();
        for id in uninitialized_ids {
            if let Some(mut status) = self.trigger_status.remove(&id) {
                let fire_now = match status.config.start_block {
                    Some(start_block) => block_height == start_block.get(),
                    None => true, // No start block means fire immediately
                };

                self.uninitialized_triggers.remove(&id);

                if fire_now {
                    // Fire now
                    to_fire.push(id);

                    // Schedule next
                    let next = block_height + status.config.n_blocks.get() as u64;
                    status.next_block = Some(next);
                    self.schedule_next(&status, id);
                } else {
                    // Not time yet, just initialize and wait
                    let next = if let Some(start_block) = status.config.start_block {
                        start_block.get()
                    } else {
                        block_height + status.config.n_blocks.get() as u64
                    };
                    status.next_block = Some(next);
                    self.triggers_by_block.entry(next).or_default().insert(id);
                    self.scheduled_blocks.insert(next);
                }

                // Re-store updated status
                self.trigger_status.insert(id, status);
            }
        }

        // Process all triggers scheduled for this block
        if let Some(triggers) = self.triggers_by_block.remove(&block_height) {
            self.scheduled_blocks.remove(&block_height);

            for id in triggers {
                if let Some(mut status) = self.trigger_status.remove(&id) {
                    let config = &status.config;

                    // Handle end_block expiration
                    if let Some(end_block) = config.end_block {
                        if block_height > end_block.get() {
                            continue; // expired
                        }
                    }

                    // If start_block is set, enforce strict equality
                    if let Some(start_block) = config.start_block {
                        let start_block = start_block.get();
                        if block_height < start_block {
                            // Too early — shouldn't happen
                            self.schedule_next(&status, id);
                            continue;
                        } else if block_height > start_block
                            && status.next_block == Some(start_block)
                        {
                            // Missed start — drop
                            tracing::warn!(
                                "Trigger {} missed its start_block ({} < {}) and is dropped.",
                                id,
                                start_block,
                                block_height
                            );
                            continue;
                        }
                    }

                    // ✅ Fire the trigger
                    to_fire.push(id);

                    // Schedule the next fire
                    let next_fire = block_height + config.n_blocks.get() as u64;
                    status.next_block = Some(next_fire);
                    self.schedule_next(&status, id);

                    // Save updated status
                    self.trigger_status.insert(id, status);
                }
            }
        }

        to_fire
    }

    // Adds the trigger to the schedule for its next firing block.
    fn schedule_next(&mut self, status: &TriggerState, id: LookupId) {
        if let Some(next_block) = status.next_block {
            self.triggers_by_block
                .entry(next_block)
                .or_default()
                .insert(id);
            self.scheduled_blocks.insert(next_block);
        }
    }

    /// Removes a trigger from the scheduler.
    ///
    /// # Arguments
    /// * `lookup_id` - ID of the trigger to remove
    ///
    /// # Returns
    /// * `true` if the trigger was found and removed
    /// * `false` if the trigger didn't exist
    pub fn remove_trigger(&mut self, lookup_id: LookupId) -> bool {
        // Remove from uninitialized set if present
        self.uninitialized_triggers.remove(&lookup_id);

        // If initialized, remove from scheduled blocks
        if let Some(status) = self.trigger_status.get(&lookup_id) {
            if let Some(next_block) = status.next_block {
                if let Some(block_triggers) = self.triggers_by_block.get_mut(&next_block) {
                    block_triggers.remove(&lookup_id);

                    // Clean up the block if empty
                    if block_triggers.is_empty() {
                        self.triggers_by_block.remove(&next_block);
                        self.scheduled_blocks.remove(&next_block);
                    }
                }
            }
        }

        // Remove from status map
        self.trigger_status.remove(&lookup_id).is_some()
    }

    /// Returns the number of active triggers in the scheduler.
    pub fn len(&self) -> usize {
        self.trigger_status.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Manages multiple block schedulers, one per chain, with fine-grained locking.
#[derive(Debug, Default)]
pub struct MultiChainBlockScheduler {
    /// Scheduler per chain with individual locks for better concurrency
    chain_schedulers: DashMap<ChainName, BlockScheduler>,
}

impl MultiChainBlockScheduler {
    pub fn schedule_trigger(
        &self,
        chain_name: &ChainName,
        trigger_id: LookupId,
        config: BlockTriggerConfig,
    ) -> bool {
        // Get or create the scheduler for this chain
        let mut scheduler = self.chain_schedulers.entry(chain_name.clone()).or_default();

        scheduler.schedule_trigger(trigger_id, config)
    }

    pub fn process_block(&self, chain_name: &ChainName, block_height: u64) -> Vec<LookupId> {
        if let Some(mut scheduler) = self.chain_schedulers.get_mut(chain_name) {
            scheduler.process_block(block_height)
        } else {
            Vec::new()
        }
    }

    pub fn remove_trigger(&self, chain_name: &ChainName, trigger_id: LookupId) -> bool {
        if let Some(mut scheduler) = self.chain_schedulers.get_mut(chain_name) {
            scheduler.remove_trigger(trigger_id)
        } else {
            false
        }
    }

    pub fn total_triggers(&self) -> usize {
        self.chain_schedulers
            .iter()
            .map(|scheduler| scheduler.len())
            .sum()
    }
}
