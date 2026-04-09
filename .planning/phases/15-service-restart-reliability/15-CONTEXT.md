# Phase 15: Service Restart Reliability - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure/bug fix phase — discuss skipped)

<domain>
## Phase Boundary

Services reliably restore trigger subscriptions after the WAVS process restarts. This is a reliability bug fix — race conditions in trigger stream re-subscription cause services to miss events after restart.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure/reliability fix phase. Key constraints from STATE.md:
- Must handle race conditions in trigger stream re-subscription
- No trigger events should be silently dropped during the re-subscription window
- Previously registered services must resume receiving trigger events without manual intervention

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `packages/wavs/src/subsystems/trigger/` — Trigger subsystem with stream management
- `packages/wavs/src/dispatcher.rs` — Dispatcher orchestrating subsystems
- Trigger streams: EVM, Cosmos, cron, timer, HTTP webhook streams

### Established Patterns
- Crossbeam channels for inter-subsystem communication
- Tokio async runtime for all subsystems
- Service registration persisted on disk

### Integration Points
- Dispatcher startup sequence — where services are re-registered
- Trigger manager — where streams are created and subscribed

</code_context>

<specifics>
## Specific Ideas

No specific requirements — refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — infrastructure phase.

</deferred>
