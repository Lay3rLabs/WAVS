# Phase 13: Activity Backend Pipeline - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Submission events carry tx_hash and execution result payload from aggregator to frontend. This is a pure backend plumbing phase — adding two fields (tx_hash: String, result_payload: Option<Vec<u8>> capped at 4KB) through 4 Rust touch points and 2 frontend type definitions.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from STATE.md:
- result_payload capped at 4 KB in Rust before IPC to avoid 100 MB hex blowup
- Rust struct + TypeScript interface + listeners.ts must change atomically (no compile-time link)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `packages/gui/shared/src/event.rs` — SubmissionEvent struct (add tx_hash, result_payload fields)
- `packages/wavs/src/dispatcher.rs` — DispatcherCommand::SubmissionConfirmed variant (add fields, ~line 131)
- `packages/wavs/src/subsystems/aggregator.rs` — send site (~line 638, tx_resp.tx_hash() already available)
- `app/src/types/index.ts` — TS SubmissionEvent interface (~line 108)
- `app/src/tauri/listeners.ts` — submission event listener (~line 60)

### Established Patterns
- Events use `TauriEventExt` trait for Tauri emission
- Serde rename_all = "snake_case" on event structs
- SubmissionFailedEvent already has error: String field as precedent for optional data

### Integration Points
- Aggregator → DispatcherCommand::SubmissionConfirmed → dispatcher match arm → SubmissionEvent emit
- SubmissionEvent → Tauri IPC → listeners.ts → ActivityItem in appStore

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — infrastructure phase.

</deferred>
