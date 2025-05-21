use std::collections::{BTreeMap, HashSet};

use crate::apis::trigger::TriggerError;

use super::core::LookupId;

// This is for some sort of scheduler that runs on an interval.
// It's used in WAVS for the cron and block interval triggers
// just need to give it an `impl IntervalState`
pub struct IntervalScheduler<T: IntervalTime, S: IntervalState<Time = T>> {
    // Key is the next time to run the trigger
    // and the value is a list of triggers that will run at that time
    triggers: BTreeMap<T, Vec<S>>,
    unadded_triggers: Vec<S>,
    // just to make sure we don't have duplicates
    trigger_ids: HashSet<LookupId>,
}

pub trait IntervalTime: Ord + Copy {}

pub trait IntervalState {
    /// The unit of time this scheduler works in
    type Time: IntervalTime;

    fn lookup_id(&self) -> LookupId;

    // outer option is whether or not the trigger has hit
    // inner option is the next time the trigger will hit (if it hasn't ended)
    fn interval_hit(&mut self, now: Self::Time) -> Option<Option<Self::Time>>;

    // this is called when the trigger is added to the scheduler
    // it's possible that some time has passed from when the trigger was created
    // especially in the case of an explicit start_time
    // so we need to set the kickoff time to the current time and
    // allow the possibility that the window was missed
    fn initialize(&mut self, now: Self::Time) -> Option<Self::Time>;

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
            triggers: BTreeMap::new(),
            unadded_triggers: Vec::new(),
            trigger_ids: HashSet::new(),
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
            self.trigger_ids.insert(id);
            self.unadded_triggers.push(state);
            Ok(true)
        }
    }

    /// Call this on each “tick”
    pub fn tick(&mut self, now: T) -> Vec<LookupId> {
        // Add all the unadded triggers to the scheduler
        for mut state in self.unadded_triggers.drain(..) {
            // initialization is time is "now"
            // even if that's not the configured start time
            // it's up to the specific scheduler to manage its
            // exact interval timing
            if let Some(next_time) = state.initialize(now) {
                self.triggers.entry(next_time).or_default().push(state);
            }
        }

        let mut hits = Vec::new();
        let mut re_add = Vec::new();

        // pop all the triggers that are due
        // but stop iterating when we hit a trigger that isn't due
        while let Some((next_time, _)) = self.triggers.iter().next() {
            if *next_time > now {
                break;
            }
            let (next_time, mut states) = self.triggers.pop_first().unwrap();

            // this is a bit of a wasteful allocation
            // in the case where the trigger has not been hit
            // but moving/re-adding *all* the potential states
            // gives us "clear empty keys" in the BTreeMap as well
            // which is likely a bigger performance win overall
            for mut state in states.drain(..) {
                let mut re_insert_time = match state.interval_hit(now) {
                    Some(new_next_time) => {
                        hits.push(state.lookup_id());
                        // this is the new next time as determined by the scheduler
                        // and it may be None if the trigger has ended
                        new_next_time
                    }
                    None => {
                        // yes, we are re-adding the trigger exactly as-is
                        // unless it has ended
                        Some(next_time)
                    }
                };

                // we can use the same semantics of re_insert_time
                // to denote whether the trigger has ended due to expiry
                // (this must be _after_ the interval_hit call, since we still add the hit)
                if let Some(end_time) = state.end_time() {
                    if now >= end_time {
                        re_insert_time = None;
                    }
                }

                if let Some(next_time) = re_insert_time {
                    // if the trigger has any next time, re-insert it
                    re_add.push((next_time, state));
                }
            }
        }

        for (next_time, state) in re_add {
            self.triggers.entry(next_time).or_default().push(state);
        }

        hits
    }

    /// Totally remove a trigger (called from the TriggerManager, as opposed to local expirey)
    pub fn remove_trigger(&mut self, id: LookupId) -> bool {
        let existed = self.trigger_ids.remove(&id);

        self.triggers.retain(|_, states| {
            states.retain(|state| state.lookup_id() != id);
            !states.is_empty()
        });
        self.unadded_triggers
            .retain(|state| state.lookup_id() != id);

        existed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Dummy(LookupId);
    impl IntervalTime for u32 {}
    impl IntervalState for Dummy {
        type Time = u32;
        fn lookup_id(&self) -> LookupId {
            self.0
        }
        fn interval_hit(&mut self, _now: u32) -> Option<Option<u32>> {
            None
        }
        fn initialize(&mut self, kickoff_time: u32) -> Option<u32> {
            Some(kickoff_time)
        }
        fn start_time(&self) -> Option<u32> {
            None
        }
        fn end_time(&self) -> Option<u32> {
            None
        }
    }

    // A test trigger that tracks when it's been processed but does NOT reschedule itself
    #[derive(Clone)]
    struct OneShotTrigger {
        id: LookupId,
        next_time: u32,
        processed: bool,
    }

    impl IntervalState for OneShotTrigger {
        type Time = u32;

        fn lookup_id(&self) -> LookupId {
            self.id
        }

        fn interval_hit(&mut self, _now: u32) -> Option<Option<u32>> {
            self.processed = true;
            Some(None) // Do not reschedule
        }

        fn initialize(&mut self, _kickoff_time: u32) -> Option<u32> {
            Some(self.next_time)
        }

        fn start_time(&self) -> Option<u32> {
            None
        }

        fn end_time(&self) -> Option<u32> {
            None
        }
    }

    #[test]
    fn no_duplicate_adds() {
        let mut sched = IntervalScheduler::<u32, Dummy>::new();
        let t1 = Dummy(42);
        assert!(sched.add_trigger(t1.clone()).unwrap());
        assert!(!sched.add_trigger(t1).unwrap());
    }

    #[test]
    fn tick_only_processes_up_to_current_time() {
        let mut sched = IntervalScheduler::<u32, OneShotTrigger>::new();

        // Add triggers at different time points
        let t1 = OneShotTrigger {
            id: 1,
            next_time: 10,
            processed: false,
        };
        let t2 = OneShotTrigger {
            id: 2,
            next_time: 20,
            processed: false,
        };
        let t3 = OneShotTrigger {
            id: 3,
            next_time: 30,
            processed: false,
        };
        let t4 = OneShotTrigger {
            id: 4,
            next_time: 100,
            processed: false,
        };

        sched.add_trigger(t1).unwrap();
        sched.add_trigger(t2).unwrap();
        sched.add_trigger(t3).unwrap();
        sched.add_trigger(t4).unwrap(); // This trigger should never be processed in this test

        // Move all unadded triggers to the BTreeMap by ticking at time 0
        let hits = sched.tick(0);
        assert!(hits.is_empty(), "No hits should occur at time 0");

        // Tick at time 15, which should only process t1 (scheduled for time 10)
        let hits = sched.tick(15);
        assert_eq!(hits, vec![1], "Only trigger 1 should hit at time 15");

        // Tick at time 25, which should only process t2 (scheduled for time 20)
        let hits = sched.tick(25);
        assert_eq!(hits, vec![2], "Only trigger 2 should hit at time 25");

        // Tick at time 35, which should only process t3 (scheduled for time 30)
        let hits = sched.tick(35);
        assert_eq!(hits, vec![3], "Only trigger 3 should hit at time 35");

        // Tick at time 50, which should not process any triggers
        // (t4 is scheduled for time 100, which is beyond the current time)
        let hits = sched.tick(50);
        assert!(hits.is_empty(), "No triggers should hit at time 50");

        // Tick at time 150, which should process t4 (scheduled for time 100)
        let hits = sched.tick(150);
        assert_eq!(hits, vec![4], "Only trigger 4 should hit at time 150");

        // This demonstrates that the scheduler only processes triggers up to the current time
        // and doesn't walk the entire BTreeMap
    }
}
