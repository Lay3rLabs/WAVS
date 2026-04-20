# Phase 17: rig-wasi Fork - Research

**Researched:** 2026-04-20
**Domain:** Rust WASM/WASI target compilation — patching rig-core 0.35.0 to compile on wasm32-wasip2
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Fork lives in-tree as `packages/rig-wasi`, a workspace member in the WAVS monorepo. No external git dependencies or separate repo.
- **D-02:** Track upstream via `FORK_BASIS.md` in the `packages/rig-wasi/` directory. Document the exact upstream rig-core commit hash (0.35.0 release) and each patch applied. Manual sync when rig releases new versions.
- **D-03:** Minimal compile gate only — ~300-500 lines across 6-7 files. Only fix what blocks wasm32-wasip2 compilation. No API changes, no ergonomic cleanup, no module removal.
- **D-04:** Specific patches required:
  1. Make `reqwest` optional behind a feature flag (`Cargo.toml`, `http_client.rs`, `client/mod.rs`)
  2. Make `tokio` optional, replace `tokio::sync::watch` with `futures::channel` equivalent (`Cargo.toml`, `streaming.rs`)
  3. Unify cfg detection to `target_family = "wasm"` everywhere (`wasm_compat.rs`)
  4. Fix SSE module dead zones for wasip2 (`sse.rs`)
  5. Handle `futures-timer` if transitive (uses `std::thread::sleep`)
  6. Verify `getrandom` for wasip2 (remove `wasm_js` feature if present)

### Claude's Discretion

- Exact implementation of the futures::channel replacement for tokio::sync::watch
- Whether to use `cfg(target_family = "wasm")` or introduce a `wasip2` feature flag for detection
- Cargo.toml feature gate naming (e.g., `reqwest` vs `native-http` vs `default`)
- FORK_BASIS.md format and content

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FORK-01 | rig-core compiles to wasm32-wasip2 with reqwest made optional behind a feature flag | Patch 1 details in Standard Stack section; exact Cargo.toml changes documented |
| FORK-02 | tokio `rt` feature removed; `tokio::sync::watch` replaced with `futures::channel` equivalent | Patch 2 details; AtomicBool stub rationale documented in Architecture Patterns |
| FORK-03 | cfg detection unified — `WasmCompatSend`/`WasmBoxedFuture` use `target_family = "wasm"` consistently across all modules | Patch 3 details; cfg alias strategy in Architecture Patterns |
| FORK-04 | SSE module dead zones on wasip2 fixed (both cfg branches fire correctly) | Patch 4 details; gate-entire-module strategy documented |
| FORK-05 | Fork compiles cleanly with `cargo build --target wasm32-wasip2` on a minimal test component | Verification approach in Architecture Patterns; test component structure documented |
</phase_requirements>

---

## Summary

rig-core 0.35.0 cannot compile to `wasm32-wasip2` in its upstream state. Three hard blockers exist: unconditional `reqwest` (no wasip2 support), `tokio` with the `rt` feature (requires `std::thread`), and cfg inconsistencies that create dead zones or type mismatches on the wasip2 target. All blockers are in the platform layer, not the logic layer — the agent loop, tool dispatch, and HTTP abstraction traits are already runtime-agnostic.

The fork strategy is to copy the rig-core 0.35.0 source into `packages/rig-wasi/` as an in-tree workspace member and apply six targeted patches totaling ~300-500 lines across 6-7 files. The fork makes zero API changes — the only consumer-visible difference is that reqwest is now opt-in rather than default. A `FORK_BASIS.md` pins the upstream commit hash and documents each patch for future sync.

Verification is a minimal test component (`packages/rig-wasi/tests/compile-probe/`) that imports rig-wasi and calls one async function. The component must compile with `cargo build --target wasm32-wasip2` and produce no cfg dead-code warnings. Upstream rig is not tested against WASI in CI, so there is no external gate — this fork is the gate.

**Primary recommendation:** Copy rig-core 0.35.0 source verbatim into `packages/rig-wasi/src/`, apply the six patches in sequence (each as a documented commit), and validate with a minimal `wasm32-wasip2` compile probe before considering the phase complete.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rig-core (forked) | 0.35.0 fork | Agent loop, Tool trait, CompletionModel, 20+ LLM providers | Forked baseline; exact crates.io source at this version |
| futures | 0.3.31 | `futures::channel::oneshot` or `AtomicBool` stub to replace `tokio::sync::watch` | Already in workspace; wasip2-compatible; no thread requirements |
| getrandom | 0.3.x | Random bytes for nanoid generation in rig | wasip2 has native random via `wasi:random` — no `wasm_js` flag needed |

[VERIFIED: crates.io API] rig-core 0.35.0 released 2026-04-13.
[VERIFIED: workspace Cargo.toml] futures = "0.3.31" already in workspace dependencies.
[VERIFIED: docs.rs/crate/rig-core/latest/source/Cargo.toml.orig] tokio features `["rt", "sync"]` confirmed unconditional.

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| wstd | 0.6.6 | Async executor (`block_on`) used by the test probe component | Upgrade from workspace 0.6.5; used in test component only, not in the fork itself |
| wasip2 | 1.0.3+wasi-0.2.9 | WIT bindings for test probe | Test probe needs these; upgrade from workspace 1.0.1 |

[VERIFIED: crates.io API] wstd 0.6.6 published 2026-03-12 by Bytecode Alliance.
[VERIFIED: crates.io API] wasip2 1.0.3+wasi-0.2.9 published 2026-04-17.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `target_family = "wasm"` cfg unification | Introduce `wasip2` cargo feature flag | Feature flag approach requires callers to opt in; `target_family = "wasm"` fires automatically for all WASM targets and needs no user action |
| `std::sync::atomic::AtomicBool` stub for PauseControl | Full `futures::channel::watch` replacement | PauseControl is streaming infrastructure; WASI MVP uses non-streaming completions; stub is sufficient and avoids pulling in any channel primitive with thread assumptions |
| In-tree `packages/rig-wasi/` | Separate git repo + `[patch.crates-io]` | In-tree per D-01; no external repo needed; workspace `[patch.crates-io]` not needed since it is a direct path dep |

**Installation:**

No new `cargo install` needed. The fork is a new workspace member. Workspace `Cargo.toml` gains one members entry:

```bash
# Add to [workspace] members in WAVS/Cargo.toml
"packages/rig-wasi",
```

**Version verification:**

```bash
# rig-core 0.35.0 confirmed
curl -s https://crates.io/api/v1/crates/rig-core | jq '.crate.newest_version'
# getrandom current version
npm view getrandom version 2>/dev/null || curl -s https://crates.io/api/v1/crates/getrandom | jq '.crate.newest_version'
```

---

## Architecture Patterns

### Recommended Project Structure

```
packages/rig-wasi/
├── Cargo.toml              # Fork manifest — reqwest optional, tokio sync-only
├── FORK_BASIS.md           # Upstream rev + patch log (REQUIRED, per D-02)
├── src/
│   ├── lib.rs              # Re-exports identical to upstream rig-core
│   ├── wasm_compat.rs      # PATCH 3: unified target_family = "wasm" cfg
│   ├── streaming.rs        # PATCH 2: AtomicBool stub replaces tokio::sync::watch
│   ├── sse.rs              # PATCH 4: gate entire SSE module behind cfg(not(target_family = "wasm"))
│   ├── http_client.rs      # PATCH 1: reqwest Client impl behind cfg(feature = "reqwest")
│   ├── client/
│   │   └── mod.rs          # PATCH 1: default H type conditional on reqwest feature
│   └── [all other upstream files — unmodified]
└── tests/
    └── compile-probe/      # FORK-05 verification: minimal wasm32-wasip2 component
        ├── Cargo.toml
        └── src/
            └── lib.rs
```

### Pattern 1: Reqwest Optional Feature Gate (Patch 1)

**What:** Make `reqwest` an optional dependency; remove it from `default` features.
**When to use:** Applied once to `Cargo.toml` and two source files.

```toml
# Source: docs.rs/crate/rig-core/latest/source/Cargo.toml.orig + STACK.md patch guidance
# packages/rig-wasi/Cargo.toml [dependencies]
reqwest = { version = "0.12", features = ["json", "stream", "multipart"], optional = true }

[features]
default = ["rustls"]          # reqwest removed from default
reqwest = ["dep:reqwest"]     # opt-in for native builds
```

```rust
// Source: STACK.md Patch 1 / WAVS_AGENT_IMPROVEMENTS.md investigation
// packages/rig-wasi/src/http_client.rs
#[cfg(feature = "reqwest")]
mod reqwest_client {
    use super::*;
    // ... existing reqwest Client impl ...
}
```

### Pattern 2: tokio rt Removal, PauseControl Stub (Patch 2)

**What:** Drop `rt` from tokio features; replace `tokio::sync::watch` in `streaming.rs` with an `AtomicBool` no-op stub.
**When to use:** The stub is correct for WASI MVP — streaming completions are not used (rig's non-streaming `prompt()` path is used instead).

```toml
# Source: STACK.md Patch 2
# packages/rig-wasi/Cargo.toml
tokio = { version = "1", features = ["sync"], default-features = false }
# "rt" feature REMOVED — requires std::thread, unavailable on wasip2
```

```rust
// Source: WAVS_AGENT_IMPROVEMENTS.md §Hard Blockers + PITFALLS.md §Pitfall 1
// packages/rig-wasi/src/streaming.rs — replace PauseControl
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// WASI-compatible no-op stub replacing tokio::sync::watch-based PauseControl.
/// Streaming completions are not used in the WASI execution model.
#[derive(Clone)]
pub struct PauseControl(Arc<AtomicBool>);

impl PauseControl {
    pub fn new() -> (Self, Self) {
        let flag = Arc::new(AtomicBool::new(false));
        (PauseControl(flag.clone()), PauseControl(flag))
    }
    pub fn is_paused(&self) -> bool { self.0.load(Ordering::SeqCst) }
    pub fn pause(&self) { self.0.store(true, Ordering::SeqCst); }
    pub fn resume(&self) { self.0.store(false, Ordering::SeqCst); }
}
```

### Pattern 3: Unified cfg Detection (Patch 3)

**What:** Replace all `#[cfg(all(feature = "wasm", target_arch = "wasm32"))]` in `wasm_compat.rs` with `#[cfg(target_family = "wasm")]`.
**When to use:** One file, multiple occurrences. Apply globally with a sed pass, then verify.

```rust
// Source: WAVS_AGENT_IMPROVEMENTS.md §Cfg Inconsistencies + PITFALLS.md §Pitfall 8
// packages/rig-wasi/src/wasm_compat.rs

// BEFORE (upstream — does NOT fire on wasip2 without the "wasm" feature):
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
pub trait WasmCompatSend {}

// AFTER (fires on wasm32-wasip2 automatically):
#[cfg(target_family = "wasm")]
pub trait WasmCompatSend {}

// WasmBoxedFuture already uses target_family = "wasm" — no change needed there.
// Goal: both WasmCompatSend and WasmBoxedFuture use the same condition.
```

### Pattern 4: SSE Module Dead Zone Fix (Patch 4)

**What:** Gate the entire SSE module behind `#[cfg(not(target_family = "wasm"))]` since SSE is not used in WASI.
**When to use:** Simpler than adding a third branch for wasip2-without-wasm-feature.

```rust
// Source: WAVS_AGENT_IMPROVEMENTS.md §Hard Blockers §4 + STACK.md Patch 4
// packages/rig-wasi/src/sse.rs — add at module top
#![cfg(not(target_family = "wasm"))]
// The SSE streaming consumer is not available in WASI p2.
// rig's agent loop uses the non-streaming completion path exclusively.
// Both cfg branches in upstream (native vs browser-wasm) are excluded;
// gating the whole file is cleaner than adding a third empty branch.
```

### Pattern 5: getrandom Feature Cleanup (Patch 6)

**What:** Remove `wasm_js` feature from `getrandom` dependency in Cargo.toml.

```toml
# Source: STACK.md Patch 6
# packages/rig-wasi/Cargo.toml
getrandom = { version = "0.3", default-features = true }
# wasip2 gets random via wasi:random/random.get-random-u64 natively.
# The wasm_js feature is browser-only (wasm-bindgen); it causes build errors
# on non-browser WASM and is not needed for wasip2.
```

### Pattern 6: Minimal Compile Probe (FORK-05 Verification)

**What:** A cdylib test component that imports `rig-wasi` and calls one async function. Must compile to `wasm32-wasip2` with no errors.

```toml
# packages/rig-wasi/tests/compile-probe/Cargo.toml
[package]
name = "rig-wasi-compile-probe"
edition.workspace = true
version.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
rig-wasi = { path = "../.." }
wstd = { workspace = true }

# No example_helpers — this is a pure compile gate, not a full WAVS component
```

```rust
// packages/rig-wasi/tests/compile-probe/src/lib.rs
// Source: PITFALLS.md §"Looks Done But Isn't" — cargo component build is the real gate
use wstd::runtime::block_on;

// Verify the core type compiles without Send requirement on wasm32-wasip2
fn _type_check() {
    // WasmCompatSend must NOT require Send on wasm32-wasip2
    fn _accepts_wasm_compat<T: rig_wasi::WasmCompatSend>(_: T) {}
}

// Verify block_on works with an async probe
pub fn run_probe() {
    block_on(async {
        // Minimal: just ensure the async surface compiles
        let _ = std::future::ready(42u32).await;
    });
}
```

### Pattern 7: FORK_BASIS.md Structure

```markdown
# FORK BASIS

**Upstream:** https://github.com/0xPlaygrounds/rig
**Upstream crate:** rig-core
**Upstream version:** 0.35.0
**Upstream commit:** [SHA — fill when copying source]
**Fork date:** 2026-04-20
**Fork crate name:** rig-wasi

## Patches Applied

| # | File(s) | Description | Lines changed |
|---|---------|-------------|---------------|
| P1 | Cargo.toml, http_client.rs, client/mod.rs | reqwest optional behind feature flag | ~40 |
| P2 | Cargo.toml, streaming.rs | tokio rt removed; PauseControl -> AtomicBool stub | ~30 |
| P3 | wasm_compat.rs | cfg unified to target_family = "wasm" | ~15 |
| P4 | sse.rs | SSE module gated behind cfg(not(target_family = "wasm")) | ~5 |
| P5 | [TBD — check if futures-timer in dep tree] | futures-timer clock-based replacement if transitive | ~20 |
| P6 | Cargo.toml | getrandom wasm_js feature removed | ~3 |

## Sync Strategy

When upstream rig releases a new version:
1. Run: `git diff v{OLD}..v{NEW} -- rig-core/` to see upstream changes
2. For each upstream change: does it touch a patched file? If yes, manually apply upstream change on top of patch.
3. Update this file with new upstream rev and any patch line-count changes.
4. Run compile probe: `cargo build -p rig-wasi-compile-probe --target wasm32-wasip2`

## Known Divergence

- reqwest is NOT in the default feature set (upstream default includes it)
- Streaming completions (SSE) are unavailable in WASI (whole module gated out)
- PauseControl is a no-op stub (streaming infrastructure not needed for non-streaming completions)
```

### Anti-Patterns to Avoid

- **Introducing API changes while patching:** D-03 is strict — only fix compile blockers. No convenience wrappers, no new exports, no renamed types. API changes force downstream updates in Phase 18 before Phase 17 is even stable.
- **Using `#[cfg(target_os = "wasi")]` for cfg unification:** `target_os = "wasi"` behavior differs between `wasm32-wasip1` and `wasm32-wasip2` across Rust versions. Use `target_family = "wasm"` which fires consistently on all WASM targets. [VERIFIED: PITFALLS.md §Pitfall 8]
- **Adding `wasm32-wasip2` to the test probe's `[lib]` crate-type as "bin":** The probe must be `cdylib` — WASI components are libraries, not binaries. The entry point is exported via `wit-bindgen`, not `fn main()`.
- **Assuming `cargo build --target wasm32-wasip2` success = usable component:** The linker step for WASM components is separate. Run `wasm-tools validate` on the output to confirm no unresolved thread symbols. [VERIFIED: PITFALLS.md §Pitfall 1]
- **Marking `tokio` as optional entirely:** `tokio::sync` (Mutex, RwLock) may still be needed by other rig modules. Drop only the `rt` feature; keep `sync`. [VERIFIED: STACK.md Patch 2]

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Replacing tokio::sync::watch for PauseControl | Custom async channel | `std::sync::atomic::AtomicBool` stub | PauseControl is streaming-only infrastructure; stub is 10 lines and has zero edge cases since streaming is unused in WASI MVP |
| cfg aliases for WASI detection | Complex `build.rs` with `CARGO_CFG_TARGET_*` | `#[cfg(target_family = "wasm")]` | `target_family = "wasm"` fires on all WASM targets including wasip2; no build script needed |
| Managing rig-core source as a git submodule | git submodule tracking | In-tree copy per D-01 | Submodules add checkout complexity; in-tree copy is simpler and equally trackable via `FORK_BASIS.md` + git blame |
| Testing every rig provider for WASI compatibility | Provider test matrix | Compile probe only (Phase 17 scope) | Phase 17 is a compile gate; provider API correctness is Phase 18's domain |

**Key insight:** The fork is deliberately minimal. Resist the urge to "fix" ergonomic issues or add WASI conveniences — that is Phase 18's job. Any addition beyond the six patches increases review surface and risks API drift.

---

## Common Pitfalls

### Pitfall 1: tokio rt Linker Errors Appear at Component Assembly, Not cargo build

**What goes wrong:** `cargo build --target wasm32-wasip2` succeeds (Rust is happy) but `wasm-tools component new` or `cargo component build` fails with `__wasi_thread_spawn` unresolved symbol errors.
**Why it happens:** tokio `rt` feature pulls in threading symbols that are undefined in the WASM component model. Rust doesn't catch this; the WASM linker does.
**How to avoid:** Remove `rt` from tokio features in `Cargo.toml` before any other work. Verify with: `cargo build -p rig-wasi-compile-probe --target wasm32-wasip2` AND `wasm-tools validate` on the output binary.
**Warning signs:** `cargo build` succeeds; any mention of `pthread` or `__wasi_thread_spawn` in link output.

[VERIFIED: PITFALLS.md §Pitfall 1, STACK.md Patch 2]

### Pitfall 2: cfg Inconsistency Creates Silent Type Mismatch

**What goes wrong:** After patching only `wasm_compat.rs`, the fork compiles but the LLM completion future type is `Pin<Box<dyn Future + Send>>` in some modules and `Pin<Box<dyn Future>>` (no Send) in others. The type mismatch appears in Phase 18 when composing types, not in Phase 17's compile probe.
**Why it happens:** The upstream `WasmCompatSend` uses `#[cfg(all(feature = "wasm", target_arch = "wasm32"))]` — doesn't fire on wasip2 without the `wasm` feature. Meanwhile `WasmBoxedFuture` already uses `target_family = "wasm"`. Patching only `wasm_compat.rs` is correct; the key is patching ALL occurrences in that file, not just the first one found.
**How to avoid:** After applying Patch 3, run `grep -rn 'feature = "wasm"' packages/rig-wasi/src/` and verify zero remaining hits that should be `target_family = "wasm"`.
**Warning signs:** Phase 18 compile errors mentioning `Send` bound not satisfied on futures.

[VERIFIED: WAVS_AGENT_IMPROVEMENTS.md §Cfg Inconsistencies, PITFALLS.md §Pitfall 8]

### Pitfall 3: getrandom wasm_js Feature Breaks Non-Browser WASM

**What goes wrong:** `getrandom` with `wasm_js` feature activates `wasm-bindgen` bindings for `window.crypto.getRandomValues`. On `wasm32-wasip2`, there is no JavaScript host — this fails at link time with unresolved `__wbindgen_*` symbols.
**Why it happens:** rig-core upstream uses `getrandom = { features = ["js"] }` for browser-WASM compatibility. The `js` feature is the `wasm_js` feature alias depending on getrandom version.
**How to avoid:** Remove the `js`/`wasm_js` feature from getrandom in the fork's Cargo.toml. wasip2 has native random via `wasi:random/random.get-random-u64`.
**Warning signs:** Link errors mentioning `__wbindgen_` symbols.

[VERIFIED: STACK.md Patch 6]

### Pitfall 4: futures-timer Transitive Dependency Uses std::thread::sleep

**What goes wrong:** If `futures-timer` is in the dependency tree (possibly pulled in by `futures` or another rig dep), it uses `std::thread::sleep` on non-WASM platforms. On wasip2, this fails.
**Why it happens:** `futures-timer` has WASM support for browser (`wasm32-unknown-unknown`) via `wasm-bindgen`, but no wasip2 support. It may fall through to the non-WASM path.
**How to avoid:** Check the dep tree BEFORE starting patches: `cargo tree -p rig-wasi --target wasm32-wasip2 2>/dev/null | grep futures-timer`. If present, check if it has a wasip2-compatible path; if not, remove or replace the usage in the fork.
**Warning signs:** Compile error in `futures-timer` source mentioning `std::thread::sleep`.

[VERIFIED: STACK.md Patch 5]

### Pitfall 5: Fork Adds `packages/rig-wasi` to Workspace but Misses wasm32-wasip2 Target in CI

**What goes wrong:** The workspace member compiles fine for `x86_64` (default `cargo build`). But `cargo build --target wasm32-wasip2` is only run manually. Future contributors break the WASI build without knowing it.
**Why it happens:** The workspace default target is native. WASM targets are not checked by default CI unless explicitly specified.
**How to avoid:** The justfile already has `just wasi-build-native [COMPONENT]` and `just wasi-build-docker [COMPONENT]`. After the fork is in place, the compile probe should be added as a named component for these targets. Document in `FORK_BASIS.md` that the WASI build is the canonical validation.
**Warning signs:** `cargo build` (native) passes but `just wasi-build-native rig-wasi-compile-probe` has never been run.

[ASSUMED] — CI config not inspected during this research session.

---

## Code Examples

Verified patterns from official sources and direct codebase inspection:

### Workspace Member Registration

```toml
# Source: /workspace/WAVS/Cargo.toml inspection — existing pattern
# Add to [workspace] members array:
"packages/rig-wasi",
```

### Cargo.toml for rig-wasi Package

```toml
# Source: STACK.md §Cargo Configuration + wasi-utils/Cargo.toml inspection
[package]
name = "rig-wasi"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lib]
crate-type = ["rlib"]   # NOT cdylib — this is a library, not a component

[dependencies]
# Patched: reqwest is optional
reqwest = { version = "0.12", features = ["json", "stream", "multipart"], optional = true }
# Patched: tokio rt removed
tokio = { version = "1", features = ["sync"], default-features = false }
# Patched: getrandom without wasm_js
getrandom = { version = "0.3", default-features = true }
# All other upstream deps unchanged

[features]
default = ["rustls"]          # reqwest removed from default
reqwest = ["dep:reqwest"]     # native HTTP — off for WASI builds
```

### Compile Probe Component Cargo.toml

```toml
# Source: echo-data/Cargo.toml pattern + STACK.md §WASI Component Example
[package]
name = "rig-wasi-compile-probe"
edition.workspace = true
version.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
rig-wasi = { path = "../.." }
wstd = { workspace = true }
```

### WASI Build Verification Commands

```bash
# Primary compile gate (FORK-05)
cargo build -p rig-wasi-compile-probe --target wasm32-wasip2

# Verify no unresolved thread symbols in the output
wasm-tools validate target/wasm32-wasip2/debug/rig_wasi_compile_probe.wasm

# Cross-check: browser WASM still compiles (don't break browser compat)
cargo check -p rig-wasi --target wasm32-unknown-unknown 2>&1 | head -20

# Verify reqwest is NOT in the wasip2 dep tree
cargo tree -p rig-wasi --target wasm32-wasip2 | grep reqwest
# Expected: no output (reqwest absent)

# Verify tokio rt feature is NOT in the wasip2 dep tree
cargo tree -p rig-wasi --target wasm32-wasip2 --features default | grep -A3 tokio
# Expected: tokio with sync only, no rt
```

### Checking futures-timer Before Patching

```bash
# Run this FIRST, before writing any patches
cargo tree -p rig-wasi --target wasm32-wasip2 2>/dev/null | grep futures-timer
# If no output: Patch 5 is a no-op — document "not present" in FORK_BASIS.md
# If present: investigate and apply Patch 5
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `#[cfg(all(feature = "wasm", target_arch = "wasm32"))]` | `#[cfg(target_family = "wasm")]` | Rust 1.70+ (target_family stable) | target_family fires on wasip2 without a cargo feature flag; older form requires `feature = "wasm"` to be enabled by the crate consumer |
| reqwest as default WASM transport | `HttpClientExt` trait abstraction | rig-core ~0.27+ | The abstract trait exists; upstream just hasn't made reqwest optional yet |
| tokio::sync::watch for PauseControl | AtomicBool stub for WASI | This fork | Correct for WASI MVP; streaming is out of scope per REQUIREMENTS.md |

**Deprecated/outdated:**
- `getrandom` with `js` / `wasm_js` feature: browser-only; wasip2 has native random since wasi-0.2.0

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | futures-timer may be transitive — check needed before patching | Common Pitfalls §Pitfall 4 | If present and unhandled, adds unplanned Patch 5 work; low risk since STACK.md already identified it as a "check if transitive" item |
| A2 | `cargo check --target wasm32-unknown-unknown` on the fork does not produce unexpected failures | Anti-Patterns | Browser WASM users would be broken if this fails; no browser WASM CI was run in this session |
| A3 | CI does not currently test wasm32-wasip2 builds for workspace members automatically | Pitfall 5 | If CI already covers this, Pitfall 5 is irrelevant |

---

## Open Questions

1. **Which exact upstream rig-core 0.35.0 git commit hash?**
   - What we know: Version 0.35.0 released 2026-04-13 on crates.io.
   - What's unclear: The exact commit SHA in the rig GitHub repo. Needed for `FORK_BASIS.md` D-02.
   - Recommendation: Run `cargo info rig-core` or check the rig GitHub releases page for the tag `v0.35.0` to extract the SHA before copying source.

2. **Is `futures-timer` in the rig-core 0.35.0 transitive dependency tree?**
   - What we know: STACK.md identifies it as a potential issue if transitive.
   - What's unclear: Whether it actually appears in the dep tree for the targets we care about.
   - Recommendation: Run `cargo tree -p rig-wasi --target wasm32-wasip2 | grep futures-timer` as the first action in Patch 5. If absent, document and skip.

3. **Does `tokio::sync` (Mutex, RwLock) work on `wasm32-wasip2` without `rt`?**
   - What we know: `tokio/sync` feature has its own thread requirements for some primitives. The `rt` feature definitely fails; `sync` alone may also fail for Mutex/RwLock if they use `std::thread` under the hood.
   - What's unclear: Whether the compile probe will catch this or whether it only manifests when sync primitives are actually used.
   - Recommendation: After Patch 2, check the compile probe output. If `tokio::sync` also fails on wasip2, replace with `std::sync::{Mutex, RwLock}` which are WASI-compatible, or with `futures::lock::Mutex` for async contexts.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All compilation | assumed present | workspace = 1.91.0 | — |
| wasm32-wasip2 target | FORK-05 compile probe | assumed installed | — | `rustup target add wasm32-wasip2` |
| wasm-tools | Component validation (FORK-05 verification) | [ASSUMED] | — | Skip validation step; use cargo component build instead |
| cargo | All build steps | assumed present | — | — |

[ASSUMED] wasm-tools availability not checked. The justfile uses it for component builds; likely installed.

---

## Security Domain

The fork introduces no new network calls, no new parsing, and no new cryptographic operations. It patches existing rig-core code to compile on a new target. The only security-relevant change is the getrandom patch:

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V6 Cryptography | Yes (getrandom) | Remove `wasm_js` feature; wasip2 uses `wasi:random/random.get-random-u64` which is host-provided entropy — same security model as OS `/dev/urandom` |
| V5 Input Validation | No | No new parsing code |
| V2 Authentication | No | No auth changes |

**getrandom security note:** `wasi:random/random.get-random-u64` is provided by the Wasmtime host, which delegates to the OS CSPRNG. This is equivalent to or better than the browser `crypto.getRandomValues` it replaces. No degradation. [CITED: https://docs.wasmtime.dev/api/wasmtime_wasi/]

---

## Sources

### Primary (HIGH confidence)
- `/workspace/WAVS/.planning/research/STACK.md` — Patch details, version table, Cargo.toml examples; verified against live crates.io and docs.rs
- `/workspace/WAVS/.planning/research/PITFALLS.md` — All 8 pitfalls with root causes and warning signs; verified against direct codebase inspection
- `/workspace/WAVS_AGENT_IMPROVEMENTS.md` — April 2026 investigation: hard blockers confirmed at source level (docs.rs rig-core 0.35.0 source inspection)
- `/workspace/WAVS/Cargo.toml` — Workspace dependencies: futures 0.3.31, wstd 0.6.5, wasip2 1.0.1, tokio 1.47.1

### Secondary (MEDIUM confidence)
- `github.com/seanmonstar/reqwest/issues/2979` — wasip2 support open, no merged PR (cited in STACK.md; not re-verified in this session)
- `docs.rs/rig-core/latest/src/rig/streaming.rs.html` — `use tokio::sync::watch` at line 31 (cited in STACK.md)
- `docs.rs/rig-core/latest/src/rig/wasm_compat.rs.html` — cfg inconsistency between WasmCompatSend and WasmBoxedFuture (cited in STACK.md)

### Tertiary (LOW confidence)
- A3 (CI coverage of wasm32-wasip2) — not checked in this session

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions verified via crates.io API in prior research session; workspace Cargo.toml inspected directly
- Architecture: HIGH — patch structure derived from direct docs.rs source inspection of rig-core 0.35.0
- Pitfalls: HIGH — validated against direct WAVS engine code inspection in prior session

**Research date:** 2026-04-20
**Valid until:** 2026-05-20 (rig releases ~every 2-3 weeks; re-verify if rig-core has a new release before Phase 17 starts)
