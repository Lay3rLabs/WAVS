use chrono::{DateTime, Duration, Timelike, Utc};
use wavs::subsystems::trigger::{
    error::TriggerError,
    lookup::LookupId,
    schedulers::{cron_scheduler::CronIntervalState, interval_scheduler::IntervalState},
};
use wavs_types::Timestamp;

// Helper function to create a timestamp from a datetime
fn make_timestamp(dt: DateTime<Utc>) -> Timestamp {
    Timestamp::from_datetime(dt).unwrap()
}

// Helper function to create a timestamp by adding seconds to now
fn make_timestamp_from_now_plus_seconds(seconds: i64) -> Timestamp {
    let now = Utc::now();
    let future = now + Duration::seconds(seconds);
    make_timestamp(future)
}

// Helper for constructing a CronIntervalState
fn make_state(
    lookup_id: LookupId,
    cron_expr: &str,
    start_time: Option<Timestamp>,
    end_time: Option<Timestamp>,
) -> Result<CronIntervalState, TriggerError> {
    CronIntervalState::new(lookup_id, cron_expr, start_time, end_time)
}

#[test]
fn test_create_cron_state() {
    // Test valid cron expression (with seconds)
    make_state(1, "* * * * * *", None, None).unwrap();

    // Test invalid cron expression
    let state = make_state(1, "invalid cron", None, None);
    assert!(state.is_err());
}

#[test]
fn test_initialization() {
    // Initialize a state with a "every second" schedule
    let mut state = make_state(1, "* * * * * *", None, None).unwrap();
    let now = make_timestamp(Utc::now());
    let next = state.initialize(now);

    // We should get a next trigger time
    assert!(next.is_some());

    // Next trigger time should be in the future
    let next_time = next.unwrap();
    assert!(next_time >= now);
}

#[test]
fn test_initialization_with_start_time() {
    // Create a state with a start time in the future
    let start_time = make_timestamp_from_now_plus_seconds(60);
    let mut state = make_state(1, "* * * * * *", Some(start_time), None).unwrap();

    // Initialize with current time
    let now = make_timestamp(Utc::now());
    let next = state.initialize(now);

    // We should get a next trigger time
    assert!(next.is_some(), "Should have a next trigger time");

    // Next should be at or after start_time
    let next_time = next.unwrap();
    assert!(
        next_time >= start_time,
        "Next time should be at or after start time"
    );
}

#[test]
fn test_interval_hit() {
    // Create a state with a "every second" schedule
    let mut state = make_state(1, "* * * * * *", None, None).unwrap();
    let now = make_timestamp(Utc::now());

    // Initialize the state
    let next_time = state.initialize(now).unwrap();

    // Try to hit before the scheduled time
    let before_time = now;
    let hit = state.interval_hit(before_time);
    assert!(hit.is_none(), "Should not trigger before scheduled time");

    // Try to hit at the scheduled time
    let hit = state.interval_hit(next_time);
    assert!(hit.is_some(), "Should trigger at scheduled time");

    // The hit should contain the next scheduled time
    let next_next_time = hit.unwrap();
    assert!(
        next_next_time.is_some(),
        "Should have another scheduled time"
    );
    assert!(
        next_next_time.unwrap() > next_time,
        "Next time should be later"
    );
}

#[test]
fn test_multiple_hits() {
    // Create a state with a "every second" schedule
    let mut state = make_state(1, "* * * * * *", None, None).unwrap();
    let now = make_timestamp(Utc::now());

    // Initialize the state
    let first_time = state.initialize(now).unwrap();

    // First hit
    let hit1 = state.interval_hit(first_time);
    assert!(hit1.is_some(), "First hit should be successful");
    let second_time_option = hit1.unwrap();
    assert!(
        second_time_option.is_some(),
        "Should have a second trigger time"
    );
    let second_time = second_time_option.unwrap();

    // Second hit
    let hit2 = state.interval_hit(second_time);
    assert!(hit2.is_some(), "Second hit should be successful");
    let third_time_option = hit2.unwrap();
    assert!(
        third_time_option.is_some(),
        "Should have a third trigger time"
    );
    let third_time = third_time_option.unwrap();

    // Verify times are increasing
    assert!(
        second_time > first_time,
        "Second time should be after first time"
    );
    assert!(
        third_time > second_time,
        "Third time should be after second time"
    );
}

#[test]
fn test_with_end_time() {
    // Create a state with an end time
    let now = make_timestamp(Utc::now());
    let end_time = make_timestamp_from_now_plus_seconds(60);

    let mut state = make_state(1, "* * * * * *", None, Some(end_time)).unwrap();

    // Initialize the state
    let next_time = state.initialize(now).unwrap();

    // Hit at the scheduled time
    let hit = state.interval_hit(next_time);
    assert!(hit.is_some(), "Should have a hit at the scheduled time");

    // Check end_time - it should be what we set
    let retrieved_end_time = state.end_time();
    assert_eq!(retrieved_end_time, Some(end_time));

    // Note: The actual end time check happens in the IntervalScheduler,
    // not in the IntervalState implementation
}

#[test]
fn test_specific_cron_expressions() {
    // Test a few different cron expressions

    // Every 5 minutes
    // Note: The cron format is "sec min hour day month day-of-week"
    let mut state = make_state(1, "0 */5 * * * *", None, None).unwrap();
    let now = make_timestamp(Utc::now());
    let next = state.initialize(now).unwrap();

    // Next time should be at the next 5-minute mark
    let next_datetime = next.into_datetime();
    assert_eq!(next_datetime.minute() % 5, 0);
    assert!(next_datetime > now.into_datetime());
    // Seconds should be 0
    assert_eq!(next_datetime.second(), 0);

    // Every day at noon
    let mut state = make_state(2, "0 0 12 * * *", None, None).unwrap();
    let now = make_timestamp(Utc::now());
    let next = state.initialize(now).unwrap();

    // Next time should be at noon
    let next_datetime = next.into_datetime();
    assert_eq!(next_datetime.hour(), 12);
    assert_eq!(next_datetime.minute(), 0);
    assert_eq!(next_datetime.second(), 0);
}

#[test]
fn test_cron_parsing() {
    // Test that various cron expressions can be parsed successfully
    // Note: The cron format is "sec min hour day month day-of-week"

    // Valid expressions
    let exprs = [
        "* * * * * *",        // Every second
        "0 */5 * * * *",      // Every 5 minutes (at 0 seconds)
        "0 0 12 * * *",       // Every day at noon
        "0 0 0 * * 7",        // Every Sunday at midnight
        "0 0 0 1 * *",        // First of every month
        "0 0 0 1 1 *",        // January 1st every year
        "0 0 0 * * MON",      // Every Monday at midnight
        "0 0 12 * * MON-FRI", // Weekdays at noon
    ];

    for expr in exprs {
        make_state(1, expr, None, None).unwrap();
    }

    // Invalid expressions
    let invalid_exprs = [
        "invalid",
        "60 * * * * *", // Invalid second (must be 0-59)
        "* 60 * * * *", // Invalid minute
        "* * 24 * * *", // Invalid hour
        "* * * 32 * *", // Invalid day
        "* * * * 13 *", // Invalid month
        "* * * * * 8",  // Invalid weekday (must be 1-7)
    ];

    for expr in invalid_exprs {
        let state = make_state(1, expr, None, None);
        assert!(state.is_err(), "Should have failed to parse: {}", expr);
    }
}
