---
phase: 14-activity-frontend-ux
verified: 2026-04-09T15:00:00Z
status: human_needed
score: 4/5 must-haves verified
human_verification:
  - test: "Grouped card submission visibility"
    expected: "On GroupedActivityCard, submission tx hash and decoded result are visible inline without requiring any card expansion — matching the 'without expanding' guarantee of the phase goal and ACT-03"
    why_human: "In the current implementation, the entire submission child card (including SubmissionRows) is nested inside the {expanded && (...)} gate of GroupedActivityCard. The collapsed header shows only status dots (amber/red) but not the tx hash or result. Whether this satisfies ACT-03's 'without expanding' intent for grouped cards cannot be resolved programmatically — it requires product judgment on whether the grouped-card expand is exempt from the goal."
---

# Phase 14: Activity Frontend UX Verification Report

**Phase Goal:** Users can see submission status, tx hash, and decoded result inline on activity cards without expanding
**Verified:** 2026-04-09T15:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Submission activity cards show tx hash and decoded result inline without expanding | PARTIAL | ActivityCard: SubmissionRows renders OUTSIDE expanded block (lines 314-316 of ActivityCard.tsx, before the Raw toggle at line 318). GroupedActivityCard: entire submission child card including SubmissionRows is INSIDE {expanded && (...)} gate (lines 90-191) — requires human judgment. |
| 2 | Result payloads display as pretty-printed JSON when content is valid JSON | VERIFIED | decodeResultPayload.ts lines 35-38: JSON.parse + JSON.stringify(parsed, null, 2) returns {kind: 'json', display: pretty}. ResultPreview renders with whitespace-pre-wrap pre tag and [JSON] badge. |
| 3 | Result payloads display as plain text when content is valid UTF-8 but not JSON | VERIFIED | decodeResultPayload.ts lines 40: catch path after JSON.parse returns {kind: 'text', display: text}. ResultPreview renders with break-all span and [Text] badge. |
| 4 | Result payloads display as truncated hex with byte count when UTF-8 decoding fails | VERIFIED | decodeResultPayload.ts lines 25-31: TextDecoder fatal:true throw path returns {kind: 'hex', display: truncated ? `${hexStr}… (${bytes.length} bytes)` : hexStr}. ResultPreview renders with tan-muted span and [Hex] badge. |
| 5 | Clicking the clipboard icon copies the full tx hash and shows Copied! feedback | VERIFIED | TxHashDisplay (ActivityCard.tsx lines 170-198): navigator.clipboard.writeText(hash), setCopied(true), setTimeout(() => setCopied(false), 1500). e.stopPropagation() prevents card toggle. |

**Score:** 4/5 truths verified (truth #1 is PARTIAL — verified for ActivityCard, human-needed for GroupedActivityCard)

### Deferred Items

None.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `app/src/utils/decodeResultPayload.ts` | Pure decode utility: hex -> UTF-8 -> JSON -> hex fallback | VERIFIED | File exists, 42 lines, exports DecodeResult type and decodeResultPayload function. All four decode paths implemented. |
| `app/src/components/activity/ActivityCard.tsx` | SubmissionRows, TxHashDisplay, ResultPreview sub-components | VERIFIED | 346 lines. Contains TxHashDisplay (line 170), ResultPreview (line 200), SubmissionRows (line 229, exported). SubmissionRows rendered at line 314-316 outside expand block. |
| `app/src/components/activity/GroupedActivityCard.tsx` | SubmissionRows integration in child submission card | VERIFIED | File exists. SubmissionRows imported (lines 6-11) and rendered at lines 156-160 with bgColor="bg-charcoal-darkest". Integration is inside the {expanded && (...)} block — see human verification item. |
| `app/src/components/activity/ActivityFeed.tsx` | Updated virtualizer height estimate | VERIFIED | Line 14: `const ESTIMATED_ITEM_HEIGHT = 130;` — changed from 90 as specified. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `app/src/components/activity/ActivityCard.tsx` | `app/src/utils/decodeResultPayload.ts` | `import { decodeResultPayload }` | WIRED | Line 6: `import { decodeResultPayload } from '../../utils/decodeResultPayload';` Used in ResultPreview at line 201. |
| `app/src/components/activity/GroupedActivityCard.tsx` | `app/src/components/activity/ActivityCard.tsx` | `import { SubmissionRows }` | WIRED | Lines 6-11: imports formatTimestamp, getTriggerAccent, DetailRows, SubmissionRows from './ActivityCard'. Used at lines 156-160. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|--------------------|--------|
| `ActivityCard.tsx` (SubmissionRows) | `item.txHash`, `item.resultPayload` | `app/src/tauri/listeners.ts` lines 70-71: `txHash: payload.tx_hash`, `resultPayload: payload.result_payload` from live Tauri IPC SubmissionEvent | Yes — populated from real Tauri IPC events, not hardcoded | FLOWING |
| `GroupedActivityCard.tsx` (SubmissionRows) | `group.submission.txHash`, `group.submission.resultPayload` | Same ActivityItem objects from store, populated by same listener pipeline | Yes | FLOWING |

### Behavioral Spot-Checks

Step 7b: TypeScript compilation verified as the primary runnable check.

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| TypeScript compiles without errors | `cd /workspace/app && node_modules/.bin/tsc --noEmit` | No output (zero errors) | PASS |
| Commits from summary exist in git history | `git log --oneline | grep -E "89b7af23|d4a3f2ea"` | Both hashes found: `89b7af23 feat(14-01): add decodeResultPayload utility`, `d4a3f2ea feat(14-01): add SubmissionRows inline display to activity cards` | PASS |

App UI checks (visual rendering, clipboard, interactive copy feedback) require running the Tauri app — deferred to human verification.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| ACT-03 | 14-01-PLAN.md | Activity cards show submission info (status, tx hash, result) inline without requiring expand | PARTIAL | Fully satisfied for standalone ActivityCard. For GroupedActivityCard, SubmissionRows is inside the expand gate — tx hash and result require expanding the group card. Status dots on collapsed header provide partial status visibility. Human verification needed. |
| ACT-04 | 14-01-PLAN.md | Result payloads decode intelligently: hex string to UTF-8 to JSON pretty-print to hex fallback | SATISFIED | decodeResultPayload.ts implements all four decode paths. Three format badges (JSON/Text/Hex) render inline. TextDecoder with fatal:true, Math.floor for byte array, JSON.stringify pretty-print all confirmed. |

No orphaned requirements — both ACT-03 and ACT-04 are mapped to Phase 14 in REQUIREMENTS.md and claimed in 14-01-PLAN.md.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found | — | — | — | — |

Checked all four modified files for TODO/FIXME, return null/empty-array stubs, placeholder text, and hardcoded empty props. No anti-patterns found. The `return null` at ResultPreview line 203 is a legitimate early return when no payload exists — not a stub, as the component is correctly gated.

### Human Verification Required

#### 1. GroupedActivityCard: Submission Info Visibility Without Expansion

**Test:** Open the WAVS desktop app. Trigger a workflow that completes with a submission. Find the grouped activity card for that event. Without clicking to expand the card, check if the tx hash and decoded result are visible in the collapsed card header or body.

**Expected (goal intent):** The collapsed grouped card should show the submission tx hash (truncated with clipboard icon) and the decoded result inline — or at minimum, the phase goal "without expanding" is met because the group-card expand is a different UX action than the Raw section expand.

**Why human:** In the code, GroupedActivityCard's entire `{group.submission && (...)}` block is gated behind `{expanded && (...)}` (line 90 of GroupedActivityCard.tsx). The collapsed header shows only: Trigger pill, trigger type pill, pending/failed status dot, and timestamp. No tx hash or result is visible without clicking to expand the group card. This may or may not satisfy ACT-03 depending on whether the design intent was: (a) inline means "without expanding to Raw JSON" (the Raw toggle), in which case both cards satisfy the goal within their own expand states, or (b) inline means "visible on the collapsed card surface", which ActivityCard satisfies but GroupedActivityCard does not. The UI-SPEC line 170 says "Always visible inline (no expand required) — satisfies ACT-03" but this language appears in the context of the standalone card interaction contract, not the grouped card. Product judgment is needed.

---

### Gaps Summary

No hard gaps found — all artifacts exist, are substantive, and data flows from real IPC events. TypeScript compiles cleanly. The one unresolved item is whether the GroupedActivityCard's design (submission detail requires group-card expand) satisfies or violates ACT-03's "without expanding" guarantee. This is a design intent question requiring human review, not a code defect.

If human verification determines GroupedActivityCard violates ACT-03, the fix is to move the `{group.submission && (...)}` block (lines 129-189 of GroupedActivityCard.tsx) outside the `{expanded && (...)}` gate, placing it after the service/workflow row and before the expand block, mirroring how ActivityCard handles it.

---

_Verified: 2026-04-09T15:00:00Z_
_Verifier: Claude (gsd-verifier)_
