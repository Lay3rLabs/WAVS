use std::num::{NonZeroU32, NonZeroU64};

use dashmap::DashMap;
use wavs_types::ChainName;

use crate::trigger_manager::lookup::LookupId;

use super::interval_scheduler::{IntervalScheduler, IntervalState, IntervalTime};

pub type BlockSchedulers = DashMap<ChainName, BlockScheduler>;

pub type BlockScheduler = IntervalScheduler<BlockHeight, BlockIntervalState>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockHeight(NonZeroU64);

impl From<NonZeroU64> for BlockHeight {
    fn from(height: NonZeroU64) -> Self {
        BlockHeight(height)
    }
}

impl IntervalTime for BlockHeight {}

#[derive(Debug)]
pub struct BlockIntervalState {
    pub interval: NonZeroU32,
    pub kickoff_time: Option<BlockHeight>,
    _lookup_id: LookupId,
    _start_time: Option<BlockHeight>,
    _end_time: Option<BlockHeight>,
}

impl BlockIntervalState {
    pub fn new(
        lookup_id: LookupId,
        interval: NonZeroU32,
        start_time: Option<BlockHeight>,
        end_time: Option<BlockHeight>,
    ) -> Self {
        Self {
            interval,
            kickoff_time: None,
            _lookup_id: lookup_id,
            _start_time: start_time,
            _end_time: end_time,
        }
    }
}

impl IntervalState for BlockIntervalState {
    type Time = BlockHeight;

    fn lookup_id(&self) -> LookupId {
        self._lookup_id
    }

    fn initialize(&mut self, now: Self::Time) -> Option<Self::Time> {
        let now = now.0.get();
        let interval = self.interval.get() as u64;

        // explicit start_time if we have it, else “now”
        let start = self._start_time.map(|height| height.0.get()).unwrap_or(now);

        // store it for interval_hit (so all future ticks use that same anchor)
        self.kickoff_time = NonZeroU64::new(start).map(BlockHeight);

        // How far past `start` we are; if now ≤ start, delta = 0
        let delta = now.saturating_sub(start);

        // How far past `start` we are, in intervals (rounded up)
        // Examples:
        //   start = 2, now = 4, interval = 3 => periods = 1
        //   start = 2, now = 5, interval = 3 => periods = 1
        //   start = 2, now = 6, interval = 3 => periods = 2
        let periods = delta.div_ceil(interval);

        // Finally, real next firing point
        // Examples:
        //   start = 2, now = 4, interval = 3 => next = 5
        //   start = 2, now = 5, interval = 3 => next = 5
        //   start = 2, now = 6, interval = 3 => next = 8
        let next = start + periods * interval;

        // convert back to BlockHeight
        NonZeroU64::new(next).map(BlockHeight)
    }

    fn interval_hit(&mut self, now: Self::Time) -> Option<Option<Self::Time>> {
        let kickoff_time = self.kickoff_time?.0.get();
        let now = now.0.get();
        let interval = self.interval.get() as u64;

        match (now - kickoff_time) % interval == 0 {
            true => {
                let next_time = now + interval;
                Some(NonZeroU64::new(next_time).map(BlockHeight))
            }
            false => None,
        }
    }

    fn start_time(&self) -> Option<Self::Time> {
        self._start_time
    }

    fn end_time(&self) -> Option<Self::Time> {
        self._end_time
    }
}
