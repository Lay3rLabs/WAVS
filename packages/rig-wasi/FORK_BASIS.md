# FORK BASIS

**Upstream:** https://github.com/0xPlaygrounds/rig
**Upstream crate:** rig-core
**Upstream version: 0.35.0**
**Upstream commit:** e759bc41b83e5e81e6ab1f143ed65288de58dcd9
**Fork date:** 2026-04-20
**Fork crate name:** rig-wasi

## Patches Applied

| # | File(s) | Description | Lines changed |
|---|---------|-------------|---------------|
| P1 | Cargo.toml, http_client/mod.rs, client/mod.rs, http_client/multipart.rs, client/model_listing.rs | reqwest optional behind feature flag; gate all reqwest impls; conditional DefaultHttpClient type | ~68 |
| P2 | streaming.rs | tokio rt removed; PauseControl -> AtomicBool stub; StreamingResult cfg unified | ~36 |
| P3 | wasm_compat.rs, agent/prompt_request/streaming.rs | cfg unified to target_family = "wasm" (was feature="wasm"+arch); StreamingResult type fixed | ~39 |
| P4 | http_client/sse.rs, lib.rs, vector_store/mod.rs, client/builder.rs | SSE module gated behind #![cfg(not(target_family = "wasm"))]; BoxedStream moved to http_client/mod.rs; providers tree gated; client/builder gated | ~18 |
| P5 | [no source changes] | futures-timer IS in dep tree (v3.0.3) but compiles cleanly on wasip2 without wasm-bindgen feature; SSE module (only user of futures_timer::Delay) is gated out via P4; no code changes needed | 0 |
| P6 | Cargo.toml | getrandom wasm_js feature removed (dep was already optional upstream; js feature not included) | ~3 |
| P-edition | Cargo.toml | Override workspace edition to "2024" — rig-core uses let-chains (Rust 2024 feature) | 1 |

## Compile Verification (FORK-05)

```bash
# Primary compile gate — passes as of 2026-04-20
cargo build -p rig-wasi-compile-probe --target wasm32-wasip2  # exit 0

# Component validation
wasm-tools validate target/wasm32-wasip2/debug/rig_wasi_compile_probe.wasm  # Validated OK

# reqwest absent from wasip2 dep tree
cargo tree -p rig-wasi --target wasm32-wasip2 | grep reqwest  # no output

# tokio sync-only (no rt)
cargo tree -p rig-wasi --target wasm32-wasip2 -f "{p} {f}" | grep tokio
# tokio v1.52.1 sync  (rt absent)
```

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
- tokio `rt` feature removed (WASI uses wstd::runtime::block_on, not a Tokio executor)
- providers tree gated behind cfg(not(target_family = "wasm")) — Phase 18 adds WASI-specific provider impls via wavs-rig crate
- edition = "2024" (workspace uses "2021"; rig-core let-chains require 2024)
