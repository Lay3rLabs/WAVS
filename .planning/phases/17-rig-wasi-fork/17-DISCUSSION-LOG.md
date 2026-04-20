# Phase 17: rig-wasi Fork - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-20
**Phase:** 17-rig-wasi-fork
**Areas discussed:** Fork location & hosting, Patch scope boundaries

---

## Fork Location & Hosting

| Option | Description | Selected |
|--------|-------------|----------|
| lay3rlabs/rig-wasi | Public fork under Layer org. Git dependency. Clear ownership. | |
| In-tree as packages/rig-wasi | Copy source into WAVS monorepo as workspace member. No external dep. | ✓ |
| Personal fork + upstream PR | Fork under personal account, submit PR upstream. Git dep until merged. | |

**User's choice:** In-tree as packages/rig-wasi
**Notes:** Keeps everything in the WAVS workspace, no external git deps to manage.

### Follow-up: Upstream tracking

| Option | Description | Selected |
|--------|-------------|----------|
| FORK_BASIS.md + manual sync | Document upstream commit and patches. Manual sync on rig releases. | ✓ |
| Git subtree merge | Pull upstream changes periodically. Cleaner merge history. | |
| You decide | Claude picks simplest approach | |

**User's choice:** FORK_BASIS.md + manual sync

---

## Patch Scope Boundaries

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal compile gate only | Only fix wasm32-wasip2 blockers. ~300-500 lines. No API changes. | ✓ |
| Minimal + ergonomic cleanup | Compile fixes plus simplify rig API for WASI, strip unused modules. | |
| Extract core only | Pull agent loop + tool dispatch (~2000 lines) into standalone crate. | |

**User's choice:** Minimal compile gate only (Recommended)
**Notes:** Keeps fork surface small, easier to maintain, easier to upstream later.

---

## Claude's Discretion

- futures::channel replacement implementation details
- cfg detection strategy (target_family vs feature flag)
- Cargo.toml feature naming
- FORK_BASIS.md format

## Deferred Ideas

None.
