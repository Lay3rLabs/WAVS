---
phase: 17-rig-wasi-fork
plan: "01"
subsystem: rig-wasi
tags: [rust, wasi, fork, rig, cargo]
dependency_graph:
  requires: []
  provides: [packages/rig-wasi workspace member with rig-core 0.35.0 source]
  affects: [Cargo.toml workspace members]
tech_stack:
  added: [rig-core 0.35.0 (forked as rig-wasi)]
  patterns: [in-tree fork, workspace member, optional feature gates]
key_files:
  created:
    - packages/rig-wasi/Cargo.toml
    - packages/rig-wasi/FORK_BASIS.md
    - packages/rig-wasi/src/ (all rig-core 0.35.0 source files)
  modified:
    - Cargo.toml (added packages/rig-wasi to workspace members)
decisions:
  - "Used reqwest 0.13 (actual upstream version) not 0.12 as research docs stated — research was based on older rig-core version"
  - "getrandom was already optional in upstream 0.35.0 — Patch 6 is just ensuring the js/wasm_js feature is not activated (no separate dep entry needed)"
  - "SSE is at http_client/sse.rs not src/sse.rs as older research suggested — correct path used in FORK_BASIS.md"
  - "Upstream git commit SHA obtained from .cargo_vcs_info.json: e759bc41b83e5e81e6ab1f143ed65288de58dcd9"
metrics:
  duration: "187 seconds (~3 minutes)"
  completed_date: "2026-04-20"
  tasks_completed: 2
  tasks_total: 2
  files_created: 149
  files_modified: 1
---

# Phase 17 Plan 01: rig-wasi Fork Scaffolding Summary

**One-liner:** rig-core 0.35.0 source copied into packages/rig-wasi with reqwest made optional, tokio rt removed, and getrandom wasm_js feature absent — prerequisite Cargo.toml patches for wasm32-wasip2 compilation.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Download rig-core 0.35.0 source and create fork crate structure | 1bc8a9e3d | packages/rig-wasi/Cargo.toml, packages/rig-wasi/src/ (148 files), Cargo.toml |
| 2 | Create FORK_BASIS.md upstream tracking document | 7a01f4226 | packages/rig-wasi/FORK_BASIS.md |

## What Was Built

- `packages/rig-wasi/` created as a new Cargo workspace member containing the verbatim rig-core 0.35.0 source tree (148 source files across all provider modules, agent, completion, streaming, embeddings, tools, etc.)
- `packages/rig-wasi/Cargo.toml` created with three WASI-critical patches applied:
  - **P1:** `reqwest = { ..., optional = true }` — removed from `default` features; new `reqwest` feature enables it opt-in
  - **P2:** `tokio = { ..., features = ["sync"], default-features = false }` — `rt` feature removed; wasip2 uses wstd::runtime::block_on
  - **P6:** getrandom `js`/`wasm_js` feature NOT enabled — wasip2 gets entropy from `wasi:random` host interface natively
- `packages/rig-wasi/FORK_BASIS.md` documents the upstream git commit SHA, all 6 planned patches, sync strategy, and known divergences

## Verification Results

All automated checks passed:
- `grep '"packages/rig-wasi"' Cargo.toml` — PASS
- `test -f packages/rig-wasi/Cargo.toml` — PASS
- `test -f packages/rig-wasi/src/lib.rs` — PASS
- `grep 'optional = true' packages/rig-wasi/Cargo.toml` matches reqwest — PASS
- `grep -v '#' packages/rig-wasi/Cargo.toml | grep tokio` shows `sync` but NOT `rt` — PASS
- `grep 'wasm_js' packages/rig-wasi/Cargo.toml` returns no matches — PASS
- `grep -q "Upstream version: 0.35.0" FORK_BASIS.md` — PASS
- `grep -q "Patches Applied" FORK_BASIS.md` — PASS
- `grep -q "Sync Strategy" FORK_BASIS.md` — PASS

## Deviations from Plan

### Auto-noted Discrepancies (No Fixes Required)

**1. [Research Discrepancy] reqwest version was 0.13 not 0.12**
- **Found during:** Task 1
- **Issue:** Research docs (17-RESEARCH.md) cited reqwest 0.12; upstream Cargo.toml.orig for rig-core 0.35.0 shows reqwest 0.13
- **Fix:** Used the actual upstream version (0.13) as specified in the downloaded source
- **Impact:** None — the fork faithfully mirrors the upstream version

**2. [Research Discrepancy] getrandom was already optional in upstream**
- **Found during:** Task 1
- **Issue:** Research implied getrandom needed to be made optional; upstream 0.35.0 already has `getrandom = { version = "0.2", optional = true }`
- **Fix:** Patch 6 documented as "js/wasm_js feature NOT activated" rather than "dep made optional"
- **Impact:** None — the critical thing is the js/wasm_js feature is absent, which is correct

**3. [Research Discrepancy] SSE file location**
- **Found during:** Task 2
- **Issue:** Research docs referenced `src/sse.rs`; actual location is `src/http_client/sse.rs`
- **Fix:** FORK_BASIS.md correctly references `http_client/sse.rs`
- **Impact:** None for Plan 01 (patches come in Plan 02)

## Known Stubs

None — this plan copies verbatim upstream source and patches only the Cargo.toml. Source-level patches (P1-P4 in FORK_BASIS.md) are applied in Plan 02. The fork will NOT compile cleanly yet — this is expected and documented in the plan's success criteria.

## Threat Flags

None — this plan only copies upstream source and creates metadata files. No new network endpoints, auth paths, or schema changes introduced.

## Self-Check: PASSED

- `packages/rig-wasi/Cargo.toml` exists: FOUND
- `packages/rig-wasi/FORK_BASIS.md` exists: FOUND
- `packages/rig-wasi/src/lib.rs` exists: FOUND
- Commit `1bc8a9e3d` exists in git log: FOUND
- Commit `7a01f4226` exists in git log: FOUND
