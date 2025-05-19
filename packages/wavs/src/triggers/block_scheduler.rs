use std::num::{NonZeroU32, NonZeroU64};

use dashmap::DashMap;
use wavs_types::ChainName;

use super::{
    core::LookupId,
    interval_scheduler::{IntervalScheduler, IntervalState, IntervalTime},
};

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

#[derive(Clone, Debug)]
pub struct BlockIntervalState {
    pub interval: NonZeroU32,
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

    fn interval_hit(&self, kickoff_time: Self::Time, now: Self::Time) -> bool {
        (now.0.get() - kickoff_time.0.get()) % self.interval.get() as u64 == 0
    }

    fn start_time(&self) -> Option<Self::Time> {
        self._start_time
    }

    fn end_time(&self) -> Option<Self::Time> {
        self._end_time
    }
}
