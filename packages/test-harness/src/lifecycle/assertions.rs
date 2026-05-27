//! Lightweight assertion helpers for typical contract-state shapes.
//!
//! These complement the standard `assert!` / `assert_eq!` macros with helpers that
//! produce richer error messages for common shapes (timeouts, near-zero deltas).

use std::fmt::Debug;

/// Assert that `actual` is within `tolerance` of `expected`. Useful for delta /
/// price values that should be "approximately zero" or "approximately equal".
pub fn assert_within<T>(actual: T, expected: T, tolerance: T)
where
    T: PartialOrd + std::ops::Sub<Output = T> + Copy + Debug,
{
    let diff = if actual > expected {
        actual - expected
    } else {
        expected - actual
    };
    assert!(
        diff <= tolerance,
        "expected {actual:?} within {tolerance:?} of {expected:?} (diff {diff:?})"
    );
}
