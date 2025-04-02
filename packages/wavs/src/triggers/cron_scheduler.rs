use chrono::Utc;
use cron::Schedule;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use wavs_types::Timestamp;

use super::core::LookupId;
use crate::apis::trigger::TriggerError;

/// Represents a scheduled cron trigger with metadata for the priority queue
#[derive(Debug, Clone, Eq)]
struct CronTriggerItem {
    lookup_id: LookupId,
    schedule: Schedule,
    next_trigger_time: Timestamp,
    start_time: Option<Timestamp>,
    end_time: Option<Timestamp>,
}

// For the binary heap, we need items with earliest trigger times at the top
// We invert the normal ordering and use lookup_id as a tiebreaker
// for deterministic ordering of same-time triggers
impl Ord for CronTriggerItem {
    fn cmp(&self, other: &Self) -> Ordering {
        match other.next_trigger_time.cmp(&self.next_trigger_time) {
            Ordering::Equal => self.lookup_id.cmp(&other.lookup_id), // Secondary ordering by ID
            other_ordering => other_ordering,
        }
    }
}

impl PartialOrd for CronTriggerItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for CronTriggerItem {
    fn eq(&self, other: &Self) -> bool {
        self.next_trigger_time == other.next_trigger_time && self.lookup_id == other.lookup_id
    }
}

/// A thread-safe scheduler for cron-based triggers
///
/// The CronScheduler maintains a priority queue of cron triggers ordered by their
/// next execution time. It provides atomic operations for adding, removing, and
/// processing triggers in a multi-threaded environment.
#[derive(Clone, Default)]
pub struct CronScheduler {
    trigger_queue: Arc<RwLock<BinaryHeap<CronTriggerItem>>>,
    trigger_lookup: Arc<RwLock<HashSet<LookupId>>>,
}

impl CronScheduler {
    pub fn new() -> Self {
        Self {
            trigger_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            trigger_lookup: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    // Add a new cron trigger
    pub fn add_trigger(
        &self,
        lookup_id: LookupId,
        schedule_str: String,
        start_time: Option<Timestamp>,
        end_time: Option<Timestamp>,
    ) -> Result<(), TriggerError> {
        let schedule = Schedule::from_str(&schedule_str).map_err(|e| TriggerError::Cron {
            expression: schedule_str.clone(),
            reason: e.to_string(),
        })?;

        // Validate time boundaries
        if let (Some(start), Some(end)) = (start_time, end_time) {
            if start > end {
                return Err(TriggerError::Cron {
                    expression: schedule_str.clone(),
                    reason: "Start time cannot be after end time".to_string(),
                });
            }
        }

        // Calculate next trigger time
        let next_trigger_time =
            schedule
                .upcoming(Utc)
                .next()
                .ok_or_else(|| TriggerError::Cron {
                    expression: schedule_str.clone(),
                    reason: "Schedule does not produce any upcoming trigger times".to_string(),
                })?;

        let next_trigger_timestamp =
            Timestamp::from_datetime(next_trigger_time).map_err(|e| TriggerError::Cron {
                expression: schedule_str.clone(),
                reason: format!("Failed to convert trigger time: {}", e),
            })?;

        // First update the lookup table
        {
            let mut lookup = self.trigger_lookup.write().unwrap();
            lookup.insert(lookup_id);
        }

        // Then update the queue
        {
            let mut queue = self.trigger_queue.write().unwrap();
            queue.push(CronTriggerItem {
                lookup_id,
                schedule,
                next_trigger_time: next_trigger_timestamp,
                start_time,
                end_time,
            });
        }

        Ok(())
    }

    // Remove a trigger
    pub fn remove_trigger(&self, lookup_id: LookupId) -> Result<(), TriggerError> {
        let mut lookup = self.trigger_lookup.write().unwrap();

        // Check if trigger exists before removing
        if !lookup.remove(&lookup_id) {
            return Err(TriggerError::NoSuchTriggerData(lookup_id));
        }

        // Queue cleanup happens during processing
        Ok(())
    }

    /// Processes all cron triggers that are due at the specified time.
    ///
    /// This method:
    /// 1. Identifies triggers that should fire at or before the given timestamp
    /// 2. Updates their next trigger time and reinserts them into the queue
    /// 3. Removes expired triggers that have passed their end time
    /// 4. Returns the lookup IDs of all triggers that should fire
    pub fn process_due_triggers(&self, now: Timestamp) -> Vec<LookupId> {
        let mut due_triggers = Vec::new();
        let mut expired_ids = Vec::new();

        // Process queue with a write lock
        {
            let mut queue = self.trigger_queue.write().unwrap();
            let lookup = self.trigger_lookup.read().unwrap();
            let mut updated_triggers = Vec::new();

            // Process only triggers that are due (at the top of the heap)
            while let Some(trigger) = queue.pop() {
                // Skip if this trigger has been removed from the lookup table
                if !lookup.contains(&trigger.lookup_id) {
                    continue; // Skip this trigger entirely
                }

                // If this trigger is not yet due, put it back and stop processing
                // (since the heap is ordered, no other triggers will be due either)
                if trigger.next_trigger_time > now {
                    queue.push(trigger);
                    break;
                }

                // Check if trigger has passed its end time
                if let Some(end_time) = trigger.end_time {
                    if now > end_time {
                        expired_ids.push(trigger.lookup_id);
                        tracing::debug!(
                            "Removing expired cron trigger ID {}: current time {} > end time {}",
                            trigger.lookup_id,
                            now.as_nanos(),
                            end_time.as_nanos()
                        );
                        continue; // Skip this trigger, don't add back to queue
                    }
                }

                // Check time boundaries to see if trigger is active
                let should_execute_now = match (trigger.start_time, trigger.end_time) {
                    (Some(start), Some(end)) => now >= start && now <= end,
                    (Some(start), None) => now >= start,
                    (None, Some(end)) => now <= end,
                    (None, None) => true, // Always active without boundaries
                };

                if should_execute_now {
                    // Trigger is due to execute
                    due_triggers.push(trigger.lookup_id);
                }

                // Calculate the next trigger time
                if let Some(next_time) = trigger.schedule.upcoming(Utc).next() {
                    if let Ok(next_timestamp) = Timestamp::from_datetime(next_time) {
                        // Check if next execution would be after end time
                        if let Some(end) = trigger.end_time {
                            if next_timestamp > end {
                                expired_ids.push(trigger.lookup_id);
                                tracing::debug!(
                                    "Removing cron trigger ID {}: next execution time {} exceeds end time {}",
                                    trigger.lookup_id,
                                    next_timestamp.as_nanos(),
                                    end.as_nanos()
                                );
                                continue; // Don't add back to queue
                            }
                        }

                        // Update for next execution and add to temporary queue
                        let mut updated_trigger = trigger;
                        updated_trigger.next_trigger_time = next_timestamp;
                        updated_triggers.push(updated_trigger);
                        continue;
                    }
                }

                // Failed to calculate next time or it's invalid - remove from queue
                expired_ids.push(trigger.lookup_id);
                tracing::debug!(
                    "Removing cron trigger ID {} - no valid next execution time",
                    trigger.lookup_id
                );
            }

            // Add all updated triggers back to the queue at once
            for trigger in updated_triggers {
                queue.push(trigger);
            }
        }

        // Now handle expired IDs with a separate write lock
        if !expired_ids.is_empty() {
            let mut lookup = self.trigger_lookup.write().unwrap();
            for id in &expired_ids {
                lookup.remove(id);
            }
        }

        due_triggers
    }
}
