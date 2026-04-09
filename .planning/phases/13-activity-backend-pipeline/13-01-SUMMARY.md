---
phase: 13-activity-backend-pipeline
plan: "01"
subsystem: submission-pipeline
tags: [rust, typescript, tauri-ipc, events, activity-feed]
dependency_graph:
  requires: []
  provides: [tx_hash-in-submission-event, result_payload-in-submission-event]
  affects: [activity-feed, tauri-ipc-shape]
tech_stack:
  added: []
  patterns: [const_hex-encode-prefixed, payload-capping-4kb]
key_files:
  created: []
  modified:
    - packages/gui/shared/src/event.rs
    - packages/wavs/src/dispatcher.rs
    - packages/wavs/src/subsystems/aggregator.rs
    - app/src/types/index.ts
    - app/src/tauri/listeners.ts
decisions:
  - "result_payload represented as Option<String> (pre-encoded hex) in SubmissionEvent to avoid dependency on private serde_helpers module in wavs_types"
  - "4096-byte cap on result_payload applied at aggregator before IPC to prevent 50MB WASM output exhausting channel memory (T-13-01 mitigation)"
metrics:
  duration: ~10 minutes
  completed: "2026-04-09"
  tasks_completed: 2
  files_modified: 5
---

# Phase 13 Plan 01: Activity Backend Pipeline — Submission Event Fields Summary

Forward tx_hash and result_payload through the Rust submission event pipeline and TypeScript type layer so the frontend receives both fields on every successful submission event.

## Tasks Completed

| Task | Description | Commit |
|------|-------------|--------|
| 1 | Add tx_hash and result_payload to 4 Rust touch points (SubmissionEvent struct, DispatcherCommand variant, match arm, aggregator send site) | 3e9295d1 |
| 2 | Update TypeScript SubmissionEvent and ActivityItem interfaces; wire fields in listeners.ts | 9c933d9f |

## What Was Built

All 6 files updated atomically so Rust serde output and TypeScript interfaces remain in sync:

**Rust changes (3 files, 4 touch points):**
- `packages/gui/shared/src/event.rs`: `SubmissionEvent` struct gains `pub tx_hash: String` and `pub result_payload: Option<String>`
- `packages/wavs/src/dispatcher.rs`: `DispatcherCommand::SubmissionConfirmed` variant gains `tx_hash: String` and `result_payload: Option<String>`; match arm destructures and forwards both fields to `SubmissionEvent` construction
- `packages/wavs/src/subsystems/aggregator.rs`: send site populates `tx_hash` from `tx_resp.tx_hash()` and `result_payload` from `submission.operator_response.payload` capped at 4096 bytes and hex-encoded via `const_hex::encode_prefixed`

**TypeScript changes (2 files):**
- `app/src/types/index.ts`: `SubmissionEvent` interface gains `tx_hash: string` and `result_payload: string | null`; `ActivityItem` interface gains `txHash?: string` and `resultPayload?: string | null`
- `app/src/tauri/listeners.ts`: submission listener forwards `txHash: payload.tx_hash` and `resultPayload: payload.result_payload` into `store.addActivity`

## Decisions Made

1. **Option<String> for result_payload**: Encoded hex string in SubmissionEvent (not `Option<Vec<u8>>`) because `serde_helpers::option_const_hex` is private to `wavs_types`. Hex encoding happens at the aggregator before entering the channel.

2. **4096-byte cap**: Applied via `raw[..raw.len().min(4096)]` before `const_hex::encode_prefixed` to mitigate T-13-01 (DoS from large WASM outputs flooding IPC channel).

## Deviations from Plan

None — plan executed exactly as written.

## Verification Results

- `cargo build -p wavs -p wavs-gui-shared` exits 0 with no errors
- `npx --prefix app tsc --noEmit` exits 0 with no errors
- `tx_hash` present in all 3 required locations (event.rs, dispatcher.rs, index.ts)
- `result_payload` present in all 4 required locations (event.rs, dispatcher.rs, aggregator.rs, index.ts)
- `4096` cap confirmed in aggregator.rs

## Known Stubs

None — all fields are wired end-to-end from aggregator to frontend listener.

## Threat Flags

None — T-13-01 (payload cap) is implemented; T-13-02 (local-only IPC, accepted risk) requires no code change.

## Self-Check: PASSED

- packages/gui/shared/src/event.rs: FOUND (contains `pub tx_hash: String`)
- packages/wavs/src/dispatcher.rs: FOUND (contains `tx_hash: String` in SubmissionConfirmed variant)
- packages/wavs/src/subsystems/aggregator.rs: FOUND (contains `const_hex::encode_prefixed` and `raw.len().min(4096)`)
- app/src/types/index.ts: FOUND (contains `tx_hash: string` in SubmissionEvent)
- app/src/tauri/listeners.ts: FOUND (contains `txHash: payload.tx_hash`)
- Commit 3e9295d1: FOUND
- Commit 9c933d9f: FOUND
