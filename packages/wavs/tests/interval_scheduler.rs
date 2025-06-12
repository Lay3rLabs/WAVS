use wavs::subsystems::trigger::{
    lookup::LookupId,
    schedulers::interval_scheduler::{IntervalScheduler, IntervalState, IntervalTime},
};

#[derive(Clone)]
struct DummyState(LookupId);

#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Debug)]
struct DummyTime(u32);

impl From<u32> for DummyTime {
    fn from(value: u32) -> Self {
        DummyTime(value)
    }
}

impl From<DummyTime> for u32 {
    fn from(value: DummyTime) -> Self {
        value.0
    }
}

impl IntervalTime for DummyTime {}

impl IntervalState for DummyState {
    type Time = DummyTime;

    fn lookup_id(&self) -> LookupId {
        self.0
    }
    fn interval_hit(&mut self, _now: Self::Time) -> Option<Option<Self::Time>> {
        None
    }
    fn initialize(&mut self, kickoff_time: Self::Time) -> Option<Self::Time> {
        Some(kickoff_time)
    }
    fn start_time(&self) -> Option<Self::Time> {
        None
    }
    fn end_time(&self) -> Option<Self::Time> {
        None
    }
}

// A test trigger that tracks when it's been processed but does NOT reschedule itself
#[derive(Clone)]
struct OneShotTrigger {
    id: LookupId,
    next_time: DummyTime,
    processed: bool,
}

impl IntervalState for OneShotTrigger {
    type Time = DummyTime;

    fn lookup_id(&self) -> LookupId {
        self.id
    }

    fn interval_hit(&mut self, _now: Self::Time) -> Option<Option<Self::Time>> {
        self.processed = true;
        Some(None) // Do not reschedule
    }

    fn initialize(&mut self, _kickoff_time: Self::Time) -> Option<Self::Time> {
        Some(self.next_time)
    }

    fn start_time(&self) -> Option<Self::Time> {
        None
    }

    fn end_time(&self) -> Option<Self::Time> {
        None
    }
}

#[test]
fn no_duplicate_adds() {
    let mut sched = IntervalScheduler::<DummyTime, DummyState>::new();
    let t1 = DummyState(42);
    assert!(sched.add_trigger(t1.clone()).unwrap());
    assert!(!sched.add_trigger(t1).unwrap());
}

#[test]
fn tick_only_processes_up_to_current_time() {
    let mut sched = IntervalScheduler::<DummyTime, OneShotTrigger>::new();

    // Add triggers at different time points
    let t1 = OneShotTrigger {
        id: 1,
        next_time: 10.into(),
        processed: false,
    };
    let t2 = OneShotTrigger {
        id: 2,
        next_time: 20.into(),
        processed: false,
    };
    let t3 = OneShotTrigger {
        id: 3,
        next_time: 30.into(),
        processed: false,
    };
    let t4 = OneShotTrigger {
        id: 4,
        next_time: 100.into(),
        processed: false,
    };

    sched.add_trigger(t1).unwrap();
    sched.add_trigger(t2).unwrap();
    sched.add_trigger(t3).unwrap();
    sched.add_trigger(t4).unwrap(); // This trigger should never be processed in this test

    // Move all unadded triggers to the BTreeMap by ticking at time 0
    let hits = sched.tick(0.into());
    assert!(hits.is_empty(), "No hits should occur at time 0");

    // Tick at time 15, which should only process t1 (scheduled for time 10)
    let hits = sched.tick(15.into());
    assert_eq!(hits, vec![1], "Only trigger 1 should hit at time 15");

    // Tick at time 25, which should only process t2 (scheduled for time 20)
    let hits = sched.tick(25.into());
    assert_eq!(hits, vec![2], "Only trigger 2 should hit at time 25");

    // Tick at time 35, which should only process t3 (scheduled for time 30)
    let hits = sched.tick(35.into());
    assert_eq!(hits, vec![3], "Only trigger 3 should hit at time 35");

    // Tick at time 50, which should not process any triggers
    // (t4 is scheduled for time 100, which is beyond the current time)
    let hits = sched.tick(50.into());
    assert!(hits.is_empty(), "No triggers should hit at time 50");

    // Tick at time 150, which should process t4 (scheduled for time 100)
    let hits = sched.tick(150.into());
    assert_eq!(hits, vec![4], "Only trigger 4 should hit at time 150");

    // This demonstrates that the scheduler only processes triggers up to the current time
    // and doesn't walk the entire BTreeMap
}
