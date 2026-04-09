# Research Summary: WAVS v1.3 Activity UX & Bug Fixes

**Researched:** 2026-04-09
**Confidence:** HIGH — all findings from direct codebase inspection

## Executive Summary

v1.3 targets four tightly scoped features: richer activity cards (tx_hash + execution result forwarded from Rust to GUI), smart result decoding (UTF-8/JSON/hex cascade), a service restart race condition fix, and a wallet settings kebab menu. All four are implementable with zero new dependencies.

## Stack Additions

**None required.** All primitives exist: `TextDecoder` (Web API), existing `hexToBytes`/`bytesToHex` helpers, `DropdownMenu` atom, and a local `HashMap` for the restart fix.

## Feature Table Stakes

- tx_hash + result_payload forwarding through SubmissionEvent pipeline
- Inline submission card visible without expanding
- Smart byte decoding (hex string -> UTF-8 -> JSON pretty -> hex fallback)
- Service restart: triggers reliably re-subscribe after process restart
- Wallet kebab menu for uncommon actions

## Key Architecture Findings

- **Data flow is one pipeline:** aggregator.rs -> DispatcherCommand::SubmissionConfirmed -> SubmissionEvent (Rust) -> Tauri IPC -> listeners.ts -> ActivityItem -> GroupedActivityCard
- **tx_hash and result_payload share all touch points** — implement together in one pass
- **Restart race:** `WatchEvmContractEvents` arrives before `StartListeningChain` completes async setup; fix with pending_watches buffer in start_watcher
- **`already_in_memory` branch** in dispatcher.rs::start() skips add_service_to_managers for settings-cache-loaded services — may independently prevent trigger re-subscription
- **Kebab menu:** pure frontend, zero Rust changes, existing DropdownMenu atom

## Watch Out For

1. **Cross-layer serialization drift:** Rust struct + TypeScript interface + listeners.ts must change atomically (no compile-time link)
2. **const_hex wire format:** result_payload arrives as hex string, not number[] — must hex-decode before UTF-8 attempt
3. **Payload size:** 50 MB payload = 100 MB hex string over IPC — cap to 4 KB in Rust emission
4. **DropdownMenu auto-close:** setIsOpen(false) fires before onClick — confirmation state must live in parent
5. **Virtualizer height:** ESTIMATED_ITEM_HEIGHT = 90 is too small for always-visible submission rows

## Suggested Build Order

1. Backend pipeline (tx_hash + result_payload) — unblocks all frontend work
2. Frontend activity UX (types + decode + card rendering) — depends on Phase 1
3. Service restart race fix — fully independent, can parallelize
4. Wallet kebab menu — pure frontend, no dependencies

---
*Synthesized: 2026-04-09*
