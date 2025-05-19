use std::collections::{HashMap, HashSet};

use crate::apis::trigger::TriggerError;

use super::core::LookupId;

// This is for some sort of scheduler that runs on an interval.
// It's used in WAVS for the cron and block interval triggers
// just need to give it an `impl Interval`
pub struct IntervalScheduler<T: IntervalTime, S: IntervalState<Time = T>> {
    // a flat vec so we can quickly iterate over it
    triggers: Vec<S>,
    unadded_triggers: Vec<S>,
    // just to make sure we don't have duplicates
    trigger_ids: HashSet<LookupId>,
    // the time from which we kick off the interval loop for each trigger
    kickoff_time: HashMap<LookupId, T>,
}

pub trait IntervalTime: Ord + Copy {}

pub trait IntervalState {
    /// The unit of time this scheduler works in
    type Time: IntervalTime;

    fn lookup_id(&self) -> LookupId;

    // this is usually just `if (now - kickoff_time) % interval == 0`
    fn interval_hit(&mut self, kickoff_time: Self::Time, now: Self::Time) -> bool;

    fn start_time(&self) -> Option<Self::Time>;

    fn end_time(&self) -> Option<Self::Time>;
}

impl<T: IntervalTime, S: IntervalState<Time = T>> Default for IntervalScheduler<T, S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: IntervalTime, S: IntervalState<Time = T>> IntervalScheduler<T, S> {
    pub fn new() -> Self {
        Self {
            triggers: Vec::new(),
            unadded_triggers: Vec::new(),
            trigger_ids: HashSet::new(),
            kickoff_time: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.triggers.len() + self.unadded_triggers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.triggers.is_empty() && self.unadded_triggers.is_empty()
    }

    /// Add a trigger, return `true` if it was added
    pub fn add_trigger(&mut self, state: S) -> std::result::Result<bool, TriggerError> {
        if let (Some(start), Some(end)) = (state.start_time(), state.end_time()) {
            if start > end {
                return Err(TriggerError::IntervalStartAfterEnd);
            }
        }

        let id = state.lookup_id();
        if self.trigger_ids.contains(&id) {
            Ok(false)
        } else {
            self.unadded_triggers.push(state);
            Ok(true)
        }
    }

    /// Call this on each “tick”
    pub fn tick(&mut self, now: T) -> Vec<LookupId> {
        let mut still_unadded = Vec::new();
        // first add any new triggers whose start time has arrived
        // or, if they don't have a start time, add them immediately
        for state in self.unadded_triggers.drain(..) {
            let kickoff_time = match state.start_time() {
                Some(st) if st > now => {
                    // hasn't started yet, just keep it and try again next time
                    still_unadded.push(state);
                    continue;
                }
                Some(st) => {
                    // start time is in the past, so we can add it now
                    st
                }
                None => {
                    // no start time, make it now
                    now
                }
            };

            self.kickoff_time.insert(state.lookup_id(), kickoff_time);
            self.triggers.push(state);
        }

        self.unadded_triggers = still_unadded;

        let mut hits = Vec::new();

        self.triggers.retain_mut(|state| {
            let kickoff_time = *self.kickoff_time.get(&state.lookup_id()).unwrap();
            if state.interval_hit(kickoff_time, now) {
                // this trigger is ready to fire
                hits.push(state.lookup_id());
            }

            // remove the trigger if it has an end time and it's past that time
            // but only AFTER checking if we hit the interval
            if let Some(end_time) = state.end_time() {
                // if the trigger has ended, remove it
                // we don't remove it from the TriggerManager
                // since this is more about expirey than full-on removal
                // and we may still want to look the trigger up by ID
                // in the manager for debugging etc.
                if now >= end_time {
                    return false;
                }
            }
            true
        });

        hits
    }

    /// Remove a trigger early
    pub fn remove_trigger(&mut self, id: LookupId) -> bool {
        let existed = self.trigger_ids.remove(&id);

        self.triggers.retain(|state| state.lookup_id() != id);
        self.unadded_triggers
            .retain(|state| state.lookup_id() != id);
        self.trigger_ids.remove(&id);
        self.kickoff_time.remove(&id);

        existed
    }
}
