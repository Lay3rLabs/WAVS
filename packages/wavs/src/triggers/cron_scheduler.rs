use std::{str::FromStr, sync::Arc};

use chrono::Utc;
use wavs_types::Timestamp;

use crate::apis::trigger::TriggerError;

use super::{
    core::LookupId,
    interval_scheduler::{IntervalScheduler, IntervalState, IntervalTime},
};

impl IntervalTime for Timestamp {}

pub type CronScheduler = Arc<std::sync::Mutex<IntervalScheduler<Timestamp, CronIntervalState>>>;

pub struct CronIntervalState {
    pub schedule: cron::Schedule,
    iterator: cron::OwnedScheduleIterator<Utc>,
    next_trigger_time: Option<Timestamp>,
    _lookup_id: LookupId,
    _start_time: Option<Timestamp>,
    _end_time: Option<Timestamp>,
}

impl CronIntervalState {
    pub fn new(
        lookup_id: LookupId,
        schedule_str: &str,
        start_time: Option<Timestamp>,
        end_time: Option<Timestamp>,
    ) -> Result<Self, TriggerError> {
        let schedule = cron::Schedule::from_str(schedule_str).map_err(|e| TriggerError::Cron {
            expression: schedule_str.to_string(),
            reason: e.to_string(),
        })?;

        // Create the iterator and next trigger time already, since
        // it lets us avoid unnecessary options and
        // gives us early error handling if the schedule is invalid
        let mut iterator = match start_time {
            Some(start_time) => schedule.after_owned(start_time.into_datetime()),
            None => schedule.upcoming_owned(Utc),
        };

        let next_trigger_time = match iterator.next().map(Timestamp::from_datetime) {
            Some(Ok(next_trigger_time)) => Some(next_trigger_time),
            Some(Err(e)) => {
                return Err(TriggerError::Cron {
                    expression: schedule_str.to_string(),
                    reason: format!("Failed to convert trigger time: {}", e),
                });
            }
            None => None,
        };

        Ok(Self {
            schedule,
            iterator,
            next_trigger_time,
            _lookup_id: lookup_id,
            _start_time: start_time,
            _end_time: end_time,
        })
    }
}

impl IntervalState for CronIntervalState {
    type Time = Timestamp;

    fn lookup_id(&self) -> LookupId {
        self._lookup_id
    }

    fn interval_hit(&mut self, _kickoff_time: Self::Time, now: Self::Time) -> bool {
        // We created the iterator already, so we can ignore kickoff_time
        if let Some(next_trigger_time) = self.next_trigger_time {
            if now >= next_trigger_time {
                // Move to the next trigger time
                match self.iterator.next().map(Timestamp::from_datetime) {
                    Some(Ok(next_trigger_time)) => {
                        self.next_trigger_time = Some(next_trigger_time);
                    }
                    Some(Err(e)) => {
                        tracing::error!("Failed to convert trigger time: {}", e);
                        self.next_trigger_time = None;
                    }
                    None => {
                        self.next_trigger_time = None;
                    }
                }

                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn start_time(&self) -> Option<Self::Time> {
        self._start_time
    }

    fn end_time(&self) -> Option<Self::Time> {
        self._end_time
    }
}
