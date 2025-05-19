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
    iterator: Option<cron::OwnedScheduleIterator<Utc>>,
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

        Ok(Self {
            schedule,
            iterator: None,
            next_trigger_time: None,
            _lookup_id: lookup_id,
            _start_time: start_time,
            _end_time: end_time,
        })
    }

    pub fn set_next_trigger_time(&mut self) {
        if let Some(iterator) = self.iterator.as_mut() {
            match iterator.next().map(Timestamp::from_datetime) {
                Some(Ok(time)) => {
                    self.next_trigger_time = Some(time);
                }
                Some(Err(e)) => {
                    tracing::error!("Failed to convert trigger time: {}", e);
                    self.next_trigger_time = None;
                }
                None => {
                    tracing::warn!("No more trigger times available");
                    self.next_trigger_time = None;
                }
            }
        }
    }
}

impl IntervalState for CronIntervalState {
    type Time = Timestamp;

    fn lookup_id(&self) -> LookupId {
        self._lookup_id
    }

    fn interval_hit(&mut self, kickoff_time: Self::Time, now: Self::Time) -> bool {
        if self.iterator.is_none() {
            self.iterator = Some(self.schedule.after_owned(kickoff_time.into_datetime()));
            self.set_next_trigger_time();
        }

        if let Some(next_trigger_time) = self.next_trigger_time {
            if now >= next_trigger_time {
                self.set_next_trigger_time();
                return true;
            }
        }

        false
    }

    fn start_time(&self) -> Option<Self::Time> {
        self._start_time
    }

    fn end_time(&self) -> Option<Self::Time> {
        self._end_time
    }
}
