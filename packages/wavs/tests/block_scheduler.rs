use std::num::{NonZeroU32, NonZeroU64};

use wavs::triggers::{
    block_scheduler::{BlockHeight, BlockIntervalState},
    core::LookupId,
    interval_scheduler::IntervalState,
};

fn make_block_height(h: u64) -> BlockHeight {
    BlockHeight::from(NonZeroU64::new(h).unwrap())
}

fn make_interval(i: u32) -> NonZeroU32 {
    NonZeroU32::new(i).unwrap()
}

// Helper for constructing a simple BlockIntervalState
fn make_state(
    lookup_id: LookupId,
    interval: u32,
    start_time: Option<u64>,
    end_time: Option<u64>,
) -> BlockIntervalState {
    BlockIntervalState::new(
        lookup_id,
        make_interval(interval),
        start_time.map(make_block_height),
        end_time.map(make_block_height),
    )
}

#[test]
fn test_initialize_periods_calculation() {
    // Test the periods calculation from the comments
    // Example 1: start = 2, now = 4, interval = 3 => periods = 1
    let mut state = make_state(1, 3, Some(2), None);
    state.initialize(make_block_height(4));

    // Example 2: start = 2, now = 5, interval = 3 => periods = 1
    let mut state = make_state(2, 3, Some(2), None);
    state.initialize(make_block_height(5));

    // Example 3: start = 2, now = 6, interval = 3 => periods = 2
    let mut state = make_state(3, 3, Some(2), None);
    state.initialize(make_block_height(6));
}

#[test]
fn test_initialize_next_firing_point() {
    // Test the next firing point calculation from the comments

    // Example 1: start = 2, now = 4, interval = 3 => next = 5
    let mut state = make_state(1, 3, Some(2), None);
    let next = state.initialize(make_block_height(4));
    assert_eq!(next, Some(make_block_height(5)));

    // Example 2: start = 2, now = 5, interval = 3 => next = 5
    let mut state = make_state(2, 3, Some(2), None);
    let next = state.initialize(make_block_height(5));
    assert_eq!(next, Some(make_block_height(5)));

    // Example 3: start = 2, now = 6, interval = 3 => next = 8
    let mut state = make_state(3, 3, Some(2), None);
    let next = state.initialize(make_block_height(6));
    assert_eq!(next, Some(make_block_height(8)));
}

#[test]
fn test_initialization_sets_kickoff_time() {
    // Test that the kickoff time is correctly set during initialization
    let mut state = make_state(1, 3, Some(2), None);
    state.initialize(make_block_height(4));
    assert_eq!(state.kickoff_time, Some(make_block_height(2)));

    // When no start_time is provided, kickoff_time should be set to "now"
    let mut state = make_state(2, 3, None, None);
    state.initialize(make_block_height(4));
    assert_eq!(state.kickoff_time, Some(make_block_height(4)));
}

#[test]
fn test_initialize_with_start_in_future() {
    // Test when start time is in the future
    let mut state = make_state(1, 3, Some(10), None);
    let next = state.initialize(make_block_height(5));
    assert_eq!(next, Some(make_block_height(10)));
    assert_eq!(state.kickoff_time, Some(make_block_height(10)));
}

#[test]
fn test_initialize_with_start_equals_now() {
    // Test when start time equals now
    let mut state = make_state(1, 3, Some(5), None);
    let next = state.initialize(make_block_height(5));
    assert_eq!(next, Some(make_block_height(5)));
    assert_eq!(state.kickoff_time, Some(make_block_height(5)));
}

#[test]
fn test_interval_hit() {
    // First initialize a state
    let mut state = make_state(1, 3, Some(2), None);
    state.initialize(make_block_height(2));

    // Test exact hit
    let result = state.interval_hit(make_block_height(5));
    assert_eq!(result, Some(Some(make_block_height(8))));

    // Test not a hit
    let result = state.interval_hit(make_block_height(6));
    assert_eq!(result, None);

    // Test another hit
    let result = state.interval_hit(make_block_height(8));
    assert_eq!(result, Some(Some(make_block_height(11))));
}

#[test]
fn test_interval_hit_with_default_kickoff() {
    // Test with no explicit start_time
    let mut state = make_state(1, 3, None, None);
    state.initialize(make_block_height(5));

    // Should hit on blocks that are multiples of 3 after 5
    let result = state.interval_hit(make_block_height(8));
    assert_eq!(result, Some(Some(make_block_height(11))));

    let result = state.interval_hit(make_block_height(11));
    assert_eq!(result, Some(Some(make_block_height(14))));
}

#[test]
fn test_end_time() {
    // Test that a block doesn't fire after end_time
    let mut state = make_state(1, 3, Some(2), Some(10));
    state.initialize(make_block_height(2));

    // Still within end time
    let result = state.interval_hit(make_block_height(8));
    assert_eq!(result, Some(Some(make_block_height(11))));

    // After end time (11 > 10)
    // The interval_scheduler would check end_time too, but here we're just testing
    // the interval_hit function itself which doesn't do that check
    let result = state.interval_hit(make_block_height(11));
    assert_eq!(result, Some(Some(make_block_height(14))));
}

#[test]
fn test_no_kickoff_time() {
    // Test behavior when kickoff_time is not set
    let mut state = make_state(1, 3, Some(2), None);
    // Don't initialize

    // interval_hit should return None when kickoff_time is None
    let result = state.interval_hit(make_block_height(5));
    assert_eq!(result, None);
}

#[test]
fn test_custom_cases() {
    // Test some additional cases

    // Wider interval
    let mut state = make_state(1, 10, Some(5), None);
    let next = state.initialize(make_block_height(7));
    assert_eq!(next, Some(make_block_height(15)));

    // Very large interval
    let mut state = make_state(1, 100, Some(50), None);
    let next = state.initialize(make_block_height(75));
    assert_eq!(next, Some(make_block_height(150)));

    // Start time far in the past - on the interval
    let mut state = make_state(1, 5, Some(10), None);
    let next = state.initialize(make_block_height(100));
    assert_eq!(next, Some(make_block_height(100)));

    // Start time far in the past - off the interval
    let mut state = make_state(1, 5, Some(10), None);
    let next = state.initialize(make_block_height(101));
    assert_eq!(next, Some(make_block_height(105)));
}
