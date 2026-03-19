# Deferred Items - Phase 5

## Pre-existing Clippy Errors in p2p.rs

The following pre-existing clippy errors exist in `packages/wavs/src/subsystems/aggregator/p2p.rs` (lines 475-1214) and are NOT caused by Phase 5 changes:

1. **doc list item without indentation** (line 475)
2. **too many arguments (9/7)** (line 476, 873)
3. **clone can be replaced with std::slice::from_ref** (lines 537, 699, 1074)
4. **non-binding let on a future** (lines 738, 841, 1113, 1214)

These should be addressed separately (not part of BLS type migration).
