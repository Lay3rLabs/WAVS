# Phase 22: Service-to-Service RPC - Context

**Gathered:** 2026-04-22
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

An agent or component can synchronously call another deployed service via `call-service`, with both the caller's `AllowedServiceCalls` and the callee's `AllowedCallers` checked before dispatch, cycle detection preventing A->B->A deadlocks, and a depth cap stopping unbounded nesting.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

</decisions>

<code_context>
## Existing Code Insights

Codebase context will be gathered during plan-phase research.

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — infrastructure phase.

</deferred>
