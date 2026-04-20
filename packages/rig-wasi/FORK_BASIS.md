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
| P1 | Cargo.toml, http_client/mod.rs, client/mod.rs | reqwest optional behind feature flag | ~40 |
| P2 | Cargo.toml, streaming.rs | tokio rt removed; PauseControl -> AtomicBool stub | ~30 |
| P3 | wasm_compat.rs | cfg unified to target_family = "wasm" | ~15 |
| P4 | http_client/sse.rs | SSE module gated behind cfg(not(target_family = "wasm")) | ~5 |
| P5 | [not present] | futures-timer replacement — checked dep tree, not transitive at wasm32-wasip2 | 0 |
| P6 | Cargo.toml | getrandom wasm_js feature removed (dep was already optional upstream; js feature not included) | ~3 |

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
