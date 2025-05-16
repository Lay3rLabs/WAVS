use std::collections::{HashMap, HashSet};

use super::core::LookupId;

// This is for some sort-of scheduler that runs on an interval.
// It's used in WAVS for the cron and block interval triggers
// just need to give it an `impl Interval`
pub struct IntervalScheduler<T: IntervalTime, S: IntervalState<Time = T>> {
    // a flat vec so we can quickly iterate over it
    pub triggers: Vec<S>,
    pub unadded_triggers: Vec<S>,
    // just to make sure we don't have duplicates
    pub trigger_ids: HashSet<LookupId>, 
    // the time from which we kick off the interval loop for each trigger
    pub kickoff_time: HashMap<LookupId, T>, 
}

pub trait IntervalTime: Ord + Copy {}

pub trait IntervalState: Clone {
    /// The unit of time this scheduler works in
    type Time: IntervalTime; 

    fn lookup_id(&self) -> LookupId;

    // this is usually just `if (now - kickoff_time) % 0`
    fn interval_hit(&self, kickoff_time: Self::Time, now: Self::Time) -> bool;

    fn start_time(&self) -> Option<Self::Time>;

    fn end_time(&self) -> Option<Self::Time>;
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

    /// Add a trigger, return `true` if it was added
    pub fn add_trigger(
        &mut self,
        state: S,
    ) -> bool {
        let id = state.lookup_id();
        if self.trigger_ids.contains(&id) {
            false
        } else {
            self.unadded_triggers.push(state);
            true
        }
    }

    /// Call this on each “tick”
    pub fn tick(&mut self, now: T) -> Vec<LookupId> {
        // first add any new triggers whose start time has arrived
        // or, if they don't have a start time, add them immediately
        self.unadded_triggers.retain(|state| {
            let kickoff_time = match state.start_time() {
                Some(st) => {
                    if st < now {
                        // hasn't started yet, just keep it and try again next time
                        return true;
                    } else {
                        st
                    }
                }
                None => {
                    // no start time, make it now
                    now
                },
            };

            self.kickoff_time.insert(state.lookup_id(), kickoff_time);
            self.triggers.push(state.clone());
            false
        });

        return self
            .triggers
            .iter()
            .filter_map(|state| {
                let kickoff_time = *self.kickoff_time.get(&state.lookup_id()).unwrap();
                if state.interval_hit(kickoff_time, now) {
                    // this trigger is ready to fire
                    Some(state.lookup_id())
                } else {
                    // this trigger is not ready yet
                    None
                }
            })
            .collect()
    }

    /// Remove a trigger early
    pub fn remove_trigger(&mut self, id: LookupId) -> bool {
        let existed = self.trigger_ids.remove(&id);

        self.triggers.retain(|state| state.lookup_id() != id);
        self.unadded_triggers.retain(|state| state.lookup_id() != id);
        self.trigger_ids.remove(&id);
        self.kickoff_time.remove(&id);

        existed
    }
}