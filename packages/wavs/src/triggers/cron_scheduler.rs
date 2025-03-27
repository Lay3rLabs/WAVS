use chrono::{DateTime, Utc};
use cron::Schedule;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use super::core::LookupId;
use crate::apis::trigger::TriggerError;

#[derive(Debug, Clone, Eq)]
struct CronTriggerItem {
    lookup_id: LookupId,
    schedule: String,
    next_trigger_time: DateTime<Utc>,
    start_time: Option<u64>,
    end_time: Option<u64>,
}

// Make comparison more specific by including lookup_id to handle same-time triggers
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

// Make CronScheduler thread-safe with proper Arc handling
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

    // Calculate next trigger time
    pub fn calculate_next_trigger(schedule_str: &str) -> Result<DateTime<Utc>, TriggerError> {
        let schedule = Schedule::from_str(schedule_str).map_err(|e| {
            TriggerError::InvalidCronExpression(schedule_str.to_string(), e.to_string())
        })?;

        let next = schedule.upcoming(Utc).next().ok_or_else(|| {
            TriggerError::InvalidCronExpression(
                schedule_str.to_string(),
                "Could not determine next trigger time".to_string(),
            )
        })?;

        Ok(next)
    }

    // Add a new cron trigger
    pub fn add_trigger(
        &self,
        lookup_id: LookupId,
        schedule: String,
        start_time: Option<u64>,
        end_time: Option<u64>,
    ) -> Result<(), TriggerError> {
        // Validate the cron schedule first
        let next_trigger_time = Self::calculate_next_trigger(&schedule)?;

        if let (Some(start), Some(end)) = (start_time, end_time) {
            if start > end {
                return Err(TriggerError::InvalidCronExpression(
                    schedule.clone(),
                    "Start time cannot be after end time".to_string(),
                ));
            }
        }

        // First update the lookup table - do this first to avoid partial updates
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
                next_trigger_time,
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

    // Process due triggers
    pub fn process_due_triggers(&self, current_time: DateTime<Utc>) -> Vec<(LookupId, String)> {
        // Take write lock on queue to modify it
        let mut due_triggers = Vec::new();
        let current_unix = current_time.timestamp() as u64;

        // Minimize lock time by scoping the queue operations
        {
            let mut queue = self.trigger_queue.write().unwrap();
            let lookup = self.trigger_lookup.read().unwrap();

            let mut pending_triggers = Vec::new();

            // Extract and process all due triggers in one pass
            while let Some(trigger) = queue.pop() {
                // Skip if this trigger has been removed
                if !lookup.contains(&trigger.lookup_id) {
                    continue;
                }

                // Check if trigger is expired based on end_time
                if let Some(end_time) = trigger.end_time {
                    if current_unix > end_time {
                        // Trigger has expired, don't add it back to the queue
                        tracing::debug!(
                            "Removing expired cron trigger ID {}: current time {} > end time {}",
                            trigger.lookup_id,
                            current_unix,
                            end_time
                        );
                        continue;
                    }
                }

                // If this trigger is in the future, put it back and stop
                if trigger.next_trigger_time > current_time {
                    pending_triggers.push(trigger);
                    break;
                }

                // Check time bounds
                let in_time_bounds = match (trigger.start_time, trigger.end_time) {
                    (Some(start), Some(end)) => current_unix >= start && current_unix <= end,
                    (Some(start), None) => current_unix >= start,
                    (None, Some(end)) => current_unix <= end,
                    (None, None) => true,
                };

                if in_time_bounds {
                    due_triggers.push((trigger.lookup_id, trigger.schedule.clone()));

                    // Calculate next trigger time
                    if let Ok(next_time) = Self::calculate_next_trigger(&trigger.schedule) {
                        pending_triggers.push(CronTriggerItem {
                            lookup_id: trigger.lookup_id,
                            schedule: trigger.schedule,
                            next_trigger_time: next_time,
                            start_time: trigger.start_time,
                            end_time: trigger.end_time,
                        });
                    }
                } else {
                    // Keep trigger if it's still within bounds but not due
                    pending_triggers.push(trigger);
                }
            }

            // Collect remaining triggers from queue
            while let Some(trigger) = queue.pop() {
                if lookup.contains(&trigger.lookup_id) {
                    pending_triggers.push(trigger);
                }
            }

            // Reinsert all pending triggers
            for trigger in pending_triggers {
                queue.push(trigger);
            }
        }

        due_triggers
    }
}
